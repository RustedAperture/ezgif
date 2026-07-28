use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct SendHistoryRepository {
    pool: SqlitePool,
}

#[async_trait::async_trait]
pub trait SendHistoryRepo: Send + Sync {
    async fn record(
        &self,
        requester_user_id: Uuid,
        bucket_id: Uuid,
        image_id: Uuid,
        visibility: &str,
    ) -> Result<(), sqlx::Error>;

    async fn has_recent_send(
        &self,
        requester_user_id: Uuid,
        image_id: Uuid,
        window_seconds: i64,
    ) -> Result<bool, sqlx::Error>;

    async fn count_recent_by_visibility(
        &self,
        requester_user_id: Uuid,
        visibility: &str,
        window_seconds: i64,
    ) -> Result<i64, sqlx::Error>;

    async fn count_for_images(
        &self,
        requester_user_id: Uuid,
        image_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i64>, sqlx::Error>;

    async fn recent_image_ids_for_buckets(
        &self,
        requester_user_id: Uuid,
        bucket_ids: &[Uuid],
        limit: usize,
    ) -> Result<Vec<Uuid>, sqlx::Error>;
}

impl SendHistoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SendHistoryRepo for SendHistoryRepository {
    async fn record(
        &self,
        requester_user_id: Uuid,
        bucket_id: Uuid,
        image_id: Uuid,
        visibility: &str,
    ) -> Result<(), sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO send_history
                (id, owner_user_id, bucket_id, image_id, bucket_name, url, response_visibility)
            SELECT ?, ?, buckets.id, images.id, buckets.name, images.url, ?
            FROM buckets
            INNER JOIN images
                ON images.id = ?
               AND images.bucket_id = buckets.id
               AND images.owner_user_id = buckets.owner_user_id
            WHERE buckets.id = ?
              AND (
                buckets.owner_user_id = ?
                OR EXISTS (
                  SELECT 1
                  FROM bucket_subscriptions ps
                  WHERE ps.bucket_id = buckets.id
                    AND ps.subscriber_user_id = ?
                    AND (
                      buckets.whitelist_enabled = 0
                      OR EXISTS (
                        SELECT 1
                        FROM bucket_whitelists w
                        WHERE w.bucket_id = buckets.id AND w.user_id = ?
                      )
                    )
                )
              )
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(requester_user_id.to_string())
        .bind(visibility)
        .bind(image_id.to_string())
        .bind(bucket_id.to_string())
        .bind(requester_user_id.to_string())
        .bind(requester_user_id.to_string())
        .bind(requester_user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() != 1 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn has_recent_send(
        &self,
        requester_user_id: Uuid,
        image_id: Uuid,
        window_seconds: i64,
    ) -> Result<bool, sqlx::Error> {
        let modifier = format!("-{window_seconds} seconds");
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM send_history
            WHERE owner_user_id = ?
              AND image_id = ?
              AND sent_at > datetime('now', ?)
            "#,
        )
        .bind(requester_user_id.to_string())
        .bind(image_id.to_string())
        .bind(modifier)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    async fn count_recent_by_visibility(
        &self,
        requester_user_id: Uuid,
        visibility: &str,
        window_seconds: i64,
    ) -> Result<i64, sqlx::Error> {
        let modifier = format!("-{window_seconds} seconds");
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM send_history
            WHERE owner_user_id = ?
              AND response_visibility = ?
              AND sent_at > datetime('now', ?)
            "#,
        )
        .bind(requester_user_id.to_string())
        .bind(visibility)
        .bind(modifier)
        .fetch_one(&self.pool)
        .await
    }

    async fn count_for_images(
        &self,
        requester_user_id: Uuid,
        image_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i64>, sqlx::Error> {
        if image_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut query = String::from(
            "SELECT image_id, COUNT(*) as count
             FROM send_history
             WHERE owner_user_id = ?
               AND image_id IN (",
        );
        for index in 0..image_ids.len() {
            if index > 0 {
                query.push_str(", ");
            }
            query.push('?');
        }
        query.push_str(") GROUP BY image_id");

        let mut built = sqlx::query_as::<_, (String, i64)>(sqlx::AssertSqlSafe(query))
            .bind(requester_user_id.to_string());
        for image_id in image_ids {
            built = built.bind(image_id.to_string());
        }

        let rows = built.fetch_all(&self.pool).await?;
        let mut counts = HashMap::new();
        for (image_id, count) in rows {
            counts.insert(
                Uuid::parse_str(&image_id).map_err(|err| sqlx::Error::Decode(Box::new(err)))?,
                count,
            );
        }

        Ok(counts)
    }

    async fn recent_image_ids_for_buckets(
        &self,
        requester_user_id: Uuid,
        bucket_ids: &[Uuid],
        limit: usize,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        if bucket_ids.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; bucket_ids.len()].join(", ");
        let sql = format!(
            "SELECT image_id FROM send_history
             WHERE owner_user_id = ?
               AND image_id IS NOT NULL
               AND bucket_id IN ({placeholders})
             ORDER BY sent_at DESC, rowid DESC
             LIMIT ?"
        );

        let mut query = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql))
            .bind(requester_user_id.to_string());
        for bucket_id in bucket_ids {
            query = query.bind(bucket_id.to_string());
        }
        query = query.bind(limit as i64);

        let rows = query.fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|id| Uuid::parse_str(&id).map_err(|err| sqlx::Error::Decode(Box::new(err))))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{
        BucketRepo, ImageRepo, UserRepo, buckets::BucketRepository, images::ImageRepository,
        users::UserRepository,
    };

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn has_recent_send_is_true_within_window_and_false_after() {
        let pool = test_pool().await;
        let users = UserRepository::new(pool.clone());
        let buckets = BucketRepository::new(pool.clone());
        let images = ImageRepository::new(pool.clone());
        let repo = SendHistoryRepository::new(pool.clone());

        let user = users
            .upsert_by_provider("discord", "owner", None, None)
            .await
            .unwrap();
        let bucket = buckets.create(user.id, "Bucket").await.unwrap();
        let image = images
            .create(user.id, bucket.id, "https://example.com/1.png")
            .await
            .unwrap();

        assert!(!repo.has_recent_send(user.id, image.id, 3).await.unwrap());

        repo.record(user.id, bucket.id, image.id, "picker")
            .await
            .unwrap();

        assert!(repo.has_recent_send(user.id, image.id, 3).await.unwrap());
        assert!(!repo.has_recent_send(user.id, image.id, 0).await.unwrap());
    }

    #[tokio::test]
    async fn count_recent_by_visibility_only_counts_matching_visibility() {
        let pool = test_pool().await;
        let users = UserRepository::new(pool.clone());
        let buckets = BucketRepository::new(pool.clone());
        let images = ImageRepository::new(pool.clone());
        let repo = SendHistoryRepository::new(pool.clone());

        let user = users
            .upsert_by_provider("discord", "owner", None, None)
            .await
            .unwrap();
        let bucket = buckets.create(user.id, "Bucket").await.unwrap();
        let image = images
            .create(user.id, bucket.id, "https://example.com/1.png")
            .await
            .unwrap();

        repo.record(user.id, bucket.id, image.id, "picker")
            .await
            .unwrap();
        repo.record(user.id, bucket.id, image.id, "public")
            .await
            .unwrap();

        let count = repo
            .count_recent_by_visibility(user.id, "picker", 60)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
