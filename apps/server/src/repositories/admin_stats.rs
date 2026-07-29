use chrono::{Days, NaiveDate, Utc};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AdminStatsRepository {
    pool: SqlitePool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminStatsSnapshot {
    pub snapshot_date: NaiveDate,
    pub user_count: i64,
    pub bucket_count: i64,
    pub image_link_count: i64,
    pub unique_file_count: Option<i64>,
    pub send_count: i64,
    pub b2_object_count: Option<i64>,
    pub b2_bytes: Option<i64>,
}

type SnapshotRow = (
    String,
    i64,
    i64,
    i64,
    Option<i64>,
    i64,
    Option<i64>,
    Option<i64>,
);

impl AdminStatsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn refresh_current_snapshot(
        &self,
        snapshot_date: NaiveDate,
    ) -> Result<AdminStatsSnapshot, sqlx::Error> {
        let snapshot = AdminStatsSnapshot {
            snapshot_date,
            user_count: total_users(&self.pool).await?,
            bucket_count: total_buckets(&self.pool).await?,
            image_link_count: total_images(&self.pool).await?,
            unique_file_count: Some(total_cdn_objects(&self.pool).await?),
            send_count: total_sends(&self.pool).await?,
            b2_object_count: None,
            b2_bytes: None,
        };

        upsert_snapshot(&self.pool, &snapshot).await?;
        self.get_snapshot(snapshot_date).await
    }

    pub async fn backfill_historical_snapshots(&self) -> Result<(), sqlx::Error> {
        let Some(mut snapshot_date) = earliest_activity_date(&self.pool).await? else {
            return Ok(());
        };
        let yesterday = Utc::now()
            .date_naive()
            .checked_sub_days(Days::new(1))
            .unwrap();

        if snapshot_date > yesterday {
            return Ok(());
        }

        loop {
            let snapshot = AdminStatsSnapshot {
                snapshot_date,
                user_count: users_through(&self.pool, snapshot_date).await?,
                bucket_count: buckets_through(&self.pool, snapshot_date).await?,
                image_link_count: images_through(&self.pool, snapshot_date).await?,
                unique_file_count: None,
                send_count: sends_through(&self.pool, snapshot_date).await?,
                b2_object_count: None,
                b2_bytes: None,
            };
            upsert_snapshot(&self.pool, &snapshot).await?;

            if snapshot_date == yesterday {
                break;
            }
            snapshot_date = snapshot_date.checked_add_days(Days::new(1)).unwrap();
        }

        Ok(())
    }

    pub async fn list_snapshots(&self) -> Result<Vec<AdminStatsSnapshot>, sqlx::Error> {
        let rows: Vec<SnapshotRow> = sqlx::query_as(
            "SELECT snapshot_date, user_count, bucket_count, image_link_count, unique_file_count, send_count, b2_object_count, b2_bytes
             FROM admin_stats_snapshots
             ORDER BY snapshot_date DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(snapshot_from_row).collect()
    }

    async fn get_snapshot(
        &self,
        snapshot_date: NaiveDate,
    ) -> Result<AdminStatsSnapshot, sqlx::Error> {
        let row: SnapshotRow = sqlx::query_as(
            "SELECT snapshot_date, user_count, bucket_count, image_link_count, unique_file_count, send_count, b2_object_count, b2_bytes
             FROM admin_stats_snapshots
             WHERE snapshot_date = ?",
        )
        .bind(snapshot_date.to_string())
        .fetch_one(&self.pool)
        .await?;

        snapshot_from_row(row)
    }
}

fn snapshot_from_row(row: SnapshotRow) -> Result<AdminStatsSnapshot, sqlx::Error> {
    let (
        snapshot_date,
        user_count,
        bucket_count,
        image_link_count,
        unique_file_count,
        send_count,
        b2_object_count,
        b2_bytes,
    ) = row;

    Ok(AdminStatsSnapshot {
        snapshot_date: NaiveDate::parse_from_str(&snapshot_date, "%Y-%m-%d")
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
        user_count,
        bucket_count,
        image_link_count,
        unique_file_count,
        send_count,
        b2_object_count,
        b2_bytes,
    })
}

async fn upsert_snapshot(
    pool: &SqlitePool,
    snapshot: &AdminStatsSnapshot,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO admin_stats_snapshots
            (snapshot_date, user_count, bucket_count, image_link_count, unique_file_count, send_count, b2_object_count, b2_bytes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(snapshot_date) DO UPDATE SET
            user_count = excluded.user_count,
            bucket_count = excluded.bucket_count,
            image_link_count = excluded.image_link_count,
            unique_file_count = COALESCE(excluded.unique_file_count, admin_stats_snapshots.unique_file_count),
            send_count = excluded.send_count,
            b2_object_count = COALESCE(excluded.b2_object_count, admin_stats_snapshots.b2_object_count),
            b2_bytes = COALESCE(excluded.b2_bytes, admin_stats_snapshots.b2_bytes)",
    )
    .bind(snapshot.snapshot_date.to_string())
    .bind(snapshot.user_count)
    .bind(snapshot.bucket_count)
    .bind(snapshot.image_link_count)
    .bind(snapshot.unique_file_count)
    .bind(snapshot.send_count)
    .bind(snapshot.b2_object_count)
    .bind(snapshot.b2_bytes)
    .execute(pool)
    .await?;

    Ok(())
}

async fn earliest_activity_date(pool: &SqlitePool) -> Result<Option<NaiveDate>, sqlx::Error> {
    let activity_date: Option<String> = sqlx::query_scalar(
        "SELECT MIN(activity_date) FROM (
            SELECT date(created_at) AS activity_date FROM users
            UNION ALL
            SELECT date(created_at) AS activity_date FROM buckets
            UNION ALL
            SELECT date(created_at) AS activity_date FROM images
            UNION ALL
            SELECT date(sent_at) AS activity_date FROM send_history
        )",
    )
    .fetch_one(pool)
    .await?;

    match activity_date {
        Some(activity_date) => NaiveDate::parse_from_str(&activity_date, "%Y-%m-%d")
            .map(Some)
            .map_err(|error| sqlx::Error::Decode(Box::new(error))),
        None => Ok(None),
    }
}

async fn total_users(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
}

async fn total_buckets(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM buckets")
        .fetch_one(pool)
        .await
}

async fn total_images(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM images")
        .fetch_one(pool)
        .await
}

async fn total_cdn_objects(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM cdn_objects")
        .fetch_one(pool)
        .await
}

async fn total_sends(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM send_history")
        .fetch_one(pool)
        .await
}

async fn users_through(pool: &SqlitePool, snapshot_date: NaiveDate) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE date(created_at) <= ?")
        .bind(snapshot_date.to_string())
        .fetch_one(pool)
        .await
}

async fn buckets_through(pool: &SqlitePool, snapshot_date: NaiveDate) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM buckets WHERE date(created_at) <= ?")
        .bind(snapshot_date.to_string())
        .fetch_one(pool)
        .await
}

async fn images_through(pool: &SqlitePool, snapshot_date: NaiveDate) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM images WHERE date(created_at) <= ?")
        .bind(snapshot_date.to_string())
        .fetch_one(pool)
        .await
}

async fn sends_through(pool: &SqlitePool, snapshot_date: NaiveDate) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM send_history WHERE date(sent_at) <= ?")
        .bind(snapshot_date.to_string())
        .fetch_one(pool)
        .await
}
