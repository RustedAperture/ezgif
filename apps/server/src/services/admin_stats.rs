use crate::repositories::admin_stats::{AdminStatsRepository, AdminStatsSnapshot};
use crate::services::storage::StorageService;
use chrono::NaiveDate;

#[derive(Clone)]
pub struct AdminStatsService {
    repo: AdminStatsRepository,
    storage: Option<StorageService>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshSnapshotResult {
    pub snapshot: AdminStatsSnapshot,
    pub storage_available: bool,
}

impl AdminStatsService {
    pub fn new(repo: AdminStatsRepository, storage: Option<StorageService>) -> Self {
        Self { repo, storage }
    }

    pub async fn refresh_snapshot(
        &self,
        snapshot_date: NaiveDate,
    ) -> Result<RefreshSnapshotResult, sqlx::Error> {
        let snapshot = self.repo.refresh_current_snapshot(snapshot_date).await?;
        let Some(storage) = &self.storage else {
            return Ok(RefreshSnapshotResult {
                snapshot,
                storage_available: false,
            });
        };

        match storage.list_stats().await {
            Ok(stats) => {
                sqlx::query(
                    "UPDATE admin_stats_snapshots
                     SET b2_object_count = ?, b2_bytes = ?
                     WHERE snapshot_date = ?",
                )
                .bind(stats.object_count)
                .bind(stats.bytes)
                .bind(snapshot_date.to_string())
                .execute(storage.pool())
                .await?;

                Ok(RefreshSnapshotResult {
                    snapshot: AdminStatsSnapshot {
                        b2_object_count: Some(stats.object_count),
                        b2_bytes: Some(stats.bytes),
                        ..snapshot
                    },
                    storage_available: true,
                })
            }
            Err(error) => {
                tracing::warn!(
                    snapshot_date = %snapshot_date,
                    error = %error,
                    "Admin stats storage listing failed; preserving previous B2 snapshot values"
                );
                Ok(RefreshSnapshotResult {
                    snapshot,
                    storage_available: false,
                })
            }
        }
    }

    pub async fn load_history(&self) -> Result<Vec<AdminStatsSnapshot>, sqlx::Error> {
        self.repo.list_snapshots().await
    }
}
