use chrono::{NaiveDate, Utc};
use memebucket_server::repositories::{
    BucketRepo, ImageRepo, UserRepo, admin_stats::AdminStatsRepository, buckets::BucketRepository,
    images::ImageRepository, users::UserRepository,
};
use sqlx::SqlitePool;
use uuid::Uuid;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn refresh_current_snapshot_counts_service_wide_records() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;

    let repo = AdminStatsRepository::new(pool.clone());
    let snapshot = repo
        .refresh_current_snapshot(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap())
        .await
        .unwrap();

    assert_eq!(snapshot.user_count, 2);
    assert_eq!(snapshot.bucket_count, 3);
    assert_eq!(snapshot.image_link_count, 4);
    assert_eq!(snapshot.send_count, 5);
}

#[tokio::test]
async fn refreshing_same_day_updates_one_snapshot_row() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;
    let repo = AdminStatsRepository::new(pool.clone());
    let date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();

    repo.refresh_current_snapshot(date).await.unwrap();
    insert_image_for_fixture_user(&pool).await;
    let refreshed = repo.refresh_current_snapshot(date).await.unwrap();
    let rows = repo.list_snapshots().await.unwrap();

    assert_eq!(
        rows.iter().filter(|row| row.snapshot_date == date).count(),
        1
    );
    assert_eq!(refreshed.image_link_count, 5);
}

#[tokio::test]
async fn historical_backfill_leaves_unavailable_file_metrics_null() {
    let pool = test_pool().await;
    seed_dated_stats_fixture(&pool).await;
    let repo = AdminStatsRepository::new(pool.clone());

    repo.backfill_historical_snapshots().await.unwrap();
    let rows = repo.list_snapshots().await.unwrap();
    let historical = rows
        .iter()
        .find(|row| row.snapshot_date < Utc::now().date_naive())
        .unwrap();

    assert!(historical.unique_file_count.is_none());
    assert!(historical.b2_object_count.is_none());
    assert!(historical.b2_bytes.is_none());
}

async fn seed_stats_fixture(pool: &SqlitePool) {
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images = ImageRepository::new(pool.clone());

    let owner_one = users
        .upsert_by_provider("discord", "admin-stats-owner-one", Some("Owner One"), None)
        .await
        .unwrap();
    let owner_two = users
        .upsert_by_provider("discord", "admin-stats-owner-two", Some("Owner Two"), None)
        .await
        .unwrap();

    let bucket_one = buckets
        .create(owner_one.id, "Owner One Primary")
        .await
        .unwrap();
    let bucket_two = buckets
        .create(owner_one.id, "Owner One Secondary")
        .await
        .unwrap();
    let bucket_three = buckets
        .create(owner_two.id, "Owner Two Primary")
        .await
        .unwrap();

    let image_one = images
        .create(owner_one.id, bucket_one.id, "https://example.com/1.gif")
        .await
        .unwrap();
    let image_two = images
        .create(owner_one.id, bucket_one.id, "https://example.com/2.gif")
        .await
        .unwrap();
    let image_three = images
        .create(owner_one.id, bucket_two.id, "https://example.com/3.gif")
        .await
        .unwrap();
    let image_four = images
        .create(owner_two.id, bucket_three.id, "https://example.com/4.gif")
        .await
        .unwrap();

    insert_send(
        pool,
        owner_one.id,
        bucket_one.id,
        image_one.id,
        &bucket_one.name,
        &image_one.url,
        "2026-07-28 01:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_one.id,
        bucket_one.id,
        image_two.id,
        &bucket_one.name,
        &image_two.url,
        "2026-07-28 02:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_one.id,
        bucket_two.id,
        image_three.id,
        &bucket_two.name,
        &image_three.url,
        "2026-07-28 03:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_two.id,
        bucket_three.id,
        image_four.id,
        &bucket_three.name,
        &image_four.url,
        "2026-07-28 04:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_two.id,
        bucket_three.id,
        image_four.id,
        &bucket_three.name,
        &image_four.url,
        "2026-07-28 05:00:00",
    )
    .await;
}

async fn insert_image_for_fixture_user(pool: &SqlitePool) {
    let (owner_id, bucket_id): (String, String) = sqlx::query_as(
        "SELECT owner_user_id, id
         FROM buckets
         WHERE name = 'Owner One Primary'",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let images = ImageRepository::new(pool.clone());
    images
        .create(
            Uuid::parse_str(&owner_id).unwrap(),
            Uuid::parse_str(&bucket_id).unwrap(),
            "https://example.com/5.gif",
        )
        .await
        .unwrap();
}

async fn seed_dated_stats_fixture(pool: &SqlitePool) {
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images = ImageRepository::new(pool.clone());

    let owner = users
        .upsert_by_provider("discord", "dated-owner", Some("Dated Owner"), None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Historical Bucket").await.unwrap();
    let image = images
        .create(owner.id, bucket.id, "https://example.com/historical.gif")
        .await
        .unwrap();

    set_timestamp(
        pool,
        "users",
        owner.id,
        "2026-07-26 12:00:00",
        Some("updated_at"),
    )
    .await;
    set_timestamp(
        pool,
        "buckets",
        bucket.id,
        "2026-07-27 09:00:00",
        Some("updated_at"),
    )
    .await;
    set_timestamp(pool, "images", image.id, "2026-07-28 10:00:00", None).await;
    insert_send(
        pool,
        owner.id,
        bucket.id,
        image.id,
        &bucket.name,
        &image.url,
        "2026-07-28 11:00:00",
    )
    .await;
}

async fn set_timestamp(
    pool: &SqlitePool,
    table: &str,
    id: Uuid,
    timestamp: &str,
    updated_column: Option<&str>,
) {
    match (table, updated_column) {
        ("users", Some("updated_at")) => {
            sqlx::query("UPDATE users SET created_at = ?, updated_at = ? WHERE id = ?")
                .bind(timestamp)
                .bind(timestamp)
                .bind(id.to_string())
                .execute(pool)
                .await
                .unwrap();
        }
        ("buckets", Some("updated_at")) => {
            sqlx::query("UPDATE buckets SET created_at = ?, updated_at = ? WHERE id = ?")
                .bind(timestamp)
                .bind(timestamp)
                .bind(id.to_string())
                .execute(pool)
                .await
                .unwrap();
        }
        ("images", None) => {
            sqlx::query("UPDATE images SET created_at = ? WHERE id = ?")
                .bind(timestamp)
                .bind(id.to_string())
                .execute(pool)
                .await
                .unwrap();
        }
        _ => panic!("unsupported timestamp target: {table} {updated_column:?}"),
    }
}

async fn insert_send(
    pool: &SqlitePool,
    owner_user_id: Uuid,
    bucket_id: Uuid,
    image_id: Uuid,
    bucket_name: &str,
    url: &str,
    sent_at: &str,
) {
    sqlx::query(
        "INSERT INTO send_history
            (id, owner_user_id, bucket_id, image_id, bucket_name, url, response_visibility, sent_at)
         VALUES (?, ?, ?, ?, ?, ?, 'public', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(owner_user_id.to_string())
    .bind(bucket_id.to_string())
    .bind(image_id.to_string())
    .bind(bucket_name)
    .bind(url)
    .bind(sent_at)
    .execute(pool)
    .await
    .unwrap();
}
