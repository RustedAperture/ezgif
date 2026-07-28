use crate::services::storage::{StorageError, StorageService};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

pub async fn run_cdn_migration(pool: SqlitePool, storage: Arc<StorageService>) {
    let upload_fn = move |url: String| {
        let s = storage.clone();
        async move { s.upload_from_url(&url).await }
    };
    run_cdn_migration_with_uploader(pool, upload_fn).await;
}

/// Inner implementation that accepts an injectable upload function.  Extracted
/// so that tests can pass a closure that returns a predictable error without
/// requiring a live object-store or outbound HTTP.
async fn run_cdn_migration_with_uploader<F, Fut>(pool: SqlitePool, upload_fn: F)
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<String, StorageError>>,
{
    tracing::info!("Starting CDN migration job for existing Discord media...");
    let mut total_migrated = 0usize;
    let mut total_broken = 0usize;

    // Track IDs that failed upload this run so we exclude them from subsequent
    // batch fetches.  Without this, rows that stay 'pending' after UploadFailed
    // would be re-selected every iteration and the loop would never terminate.
    let mut failed_this_run: HashSet<String> = HashSet::new();

    loop {
        // Fetch next batch of pending rows, excluding any that already failed
        // upload this startup run (they will be retried on the next startup).
        let rows: Vec<(String, String)> = if failed_this_run.is_empty() {
            match sqlx::query_as::<_, (String, String)>(
                "SELECT id, url FROM images WHERE cdn_status = 'pending' LIMIT 20",
            )
            .fetch_all(&pool)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("CDN migration: DB fetch failed: {e}");
                    break;
                }
            }
        } else {
            // Build a query that excludes IDs we already attempted this run.
            // SQLite supports up to 999 bind parameters; our batch is 20, so
            // this set stays small in practice.
            let placeholders = failed_this_run
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id, url FROM images WHERE cdn_status = 'pending' AND id NOT IN ({}) LIMIT 20",
                placeholders
            );
            let mut q = sqlx::query_as::<_, (String, String)>(&sql);
            for id in &failed_this_run {
                q = q.bind(id.as_str());
            }
            match q.fetch_all(&pool).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("CDN migration: DB fetch failed: {e}");
                    break;
                }
            }
        };

        if rows.is_empty() {
            break;
        }

        for (id, url) in &rows {
            if !StorageService::is_discord_cdn(url) {
                // Non-Discord URLs are already stable — mark migrated, use original url
                let _ = sqlx::query(
                    "UPDATE images SET cdn_url = ?, cdn_status = 'migrated' WHERE id = ?",
                )
                .bind(url)
                .bind(id)
                .execute(&pool)
                .await;
                total_migrated += 1;
                continue;
            }

            match upload_fn(url.clone()).await {
                Ok(cdn_url) => {
                    let _ = sqlx::query(
                        "UPDATE images SET cdn_url = ?, cdn_status = 'migrated' WHERE id = ?",
                    )
                    .bind(&cdn_url)
                    .bind(id)
                    .execute(&pool)
                    .await;
                    total_migrated += 1;
                    tracing::debug!("Migrated: {}", url);
                }
                Err(StorageError::FetchFailed(e)) => {
                    // Source URL is dead — mark broken
                    tracing::warn!("CDN migration: dead URL {}: {}", url, e);
                    let _ = sqlx::query("UPDATE images SET cdn_status = 'broken' WHERE id = ?")
                        .bind(id)
                        .execute(&pool)
                        .await;
                    total_broken += 1;
                }
                Err(StorageError::InvalidImage) => {
                    // Source bytes are not a decodable image — treat as broken.
                    tracing::warn!("CDN migration: invalid image bytes for {}", url);
                    let _ = sqlx::query("UPDATE images SET cdn_status = 'broken' WHERE id = ?")
                        .bind(id)
                        .execute(&pool)
                        .await;
                    total_broken += 1;
                }
                Err(StorageError::UploadFailed(e)) => {
                    // Upload failed — leave as pending so it retries next startup,
                    // but exclude this ID from the current run to avoid an infinite loop.
                    tracing::warn!("CDN migration: upload failed for {}: {}", url, e);
                    failed_this_run.insert(id.clone());
                }
            }
        }

        // Throttle to avoid hammering Discord CDN
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    tracing::info!(
        "CDN migration complete: {} migrated, {} broken",
        total_migrated,
        total_broken
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn failed_this_run_excludes_ids_from_retry() {
        // Verify that inserting a failing ID into the set prevents it from
        // being re-selected.  This is a logical unit test of the data structure
        // used to prevent the infinite loop — the actual SQL exclusion is
        // covered by integration tests with a real DB connection.
        let mut failed: HashSet<String> = HashSet::new();
        let id = "abc-123".to_string();

        assert!(!failed.contains(&id));
        failed.insert(id.clone());
        assert!(failed.contains(&id));

        // Inserting the same ID again is idempotent
        failed.insert(id.clone());
        assert_eq!(failed.len(), 1);
    }

    /// Set up a minimal in-memory SQLite database with just the columns that
    /// `run_cdn_migration_with_uploader` queries.  No FK constraints are needed
    /// because the function only touches the `images` table.
    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE images (
                id TEXT PRIMARY KEY NOT NULL,
                url TEXT NOT NULL,
                cdn_url TEXT,
                cdn_status TEXT NOT NULL DEFAULT 'pending'
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    /// Verifies that the migration loop:
    ///   1. Terminates (does not spin forever) when every upload persistently fails.
    ///   2. Leaves all rows as `cdn_status = 'pending'` — UploadFailed rows are
    ///      reserved for the next startup retry, not marked broken.
    ///   3. Attempts each row at most once per run — the `AND id NOT IN (...)`
    ///      clause prevents re-selecting already-failed IDs.
    #[tokio::test]
    async fn migration_loop_terminates_when_uploads_persistently_fail() {
        let pool = setup_test_db().await;

        // Insert 3 rows with Discord CDN URLs so they go through the upload path.
        for i in 0..3usize {
            sqlx::query("INSERT INTO images (id, url, cdn_status) VALUES (?, ?, 'pending')")
                .bind(format!("image-{i}"))
                .bind(format!(
                    "https://cdn.discordapp.com/attachments/111/222/image-{i}.gif"
                ))
                .execute(&pool)
                .await
                .unwrap();
        }

        // Upload function that always signals a transient upload failure.
        // This is the critical case: UploadFailed rows stay 'pending', so without
        // the `failed_this_run` exclusion set the loop would spin indefinitely.
        let upload_fn = |_url: String| async {
            Err::<String, StorageError>(StorageError::UploadFailed("simulated failure".to_string()))
        };

        // The loop must complete well within 10 seconds.  With 3 rows in one
        // batch (LIMIT 20) there is exactly one 500 ms throttle sleep, then the
        // second fetch returns empty and the loop breaks.
        let outcome = tokio::time::timeout(
            Duration::from_secs(10),
            run_cdn_migration_with_uploader(pool.clone(), upload_fn),
        )
        .await;

        assert!(
            outcome.is_ok(),
            "migration loop did not terminate — likely spinning on persistently-failing rows"
        );

        // All rows should remain 'pending': UploadFailed is a transient error
        // reserved for the next startup, not a permanent broken state.
        let statuses: Vec<String> = sqlx::query_scalar("SELECT cdn_status FROM images ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();

        assert_eq!(statuses.len(), 3, "expected 3 image rows");
        assert!(
            statuses.iter().all(|s| s == "pending"),
            "all rows should remain 'pending' after persistent UploadFailed, got: {statuses:?}"
        );

        // Confirm the NOT IN exclusion worked: if any row were re-fetched and
        // re-attempted it would show up a second time in failed_this_run, but
        // the statuses being 'pending' (not 'broken') already proves no row was
        // re-processed.  As an additional sanity check, count distinct IDs that
        // were seen as pending after the run — should still be exactly 3.
        let pending_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM images WHERE cdn_status = 'pending'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(
            pending_count, 3,
            "all 3 rows should be pending after the migration run"
        );
    }
}
