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
        self.repo.refresh_current_snapshot(snapshot_date).await?;

        let (snapshot, storage_available) = match &self.storage {
            Some(storage) => match storage.list_stats().await {
                Ok(stats) => (
                    self.repo
                        .apply_provider_metrics(
                            snapshot_date,
                            Some(stats.object_count),
                            Some(stats.bytes),
                        )
                        .await?,
                    true,
                ),
                Err(error) => {
                    tracing::warn!(
                        snapshot_date = %snapshot_date,
                        error = %error,
                        "Admin stats storage listing failed; preserving previous B2 snapshot values"
                    );
                    (
                        self.repo
                            .apply_provider_metrics(snapshot_date, None, None)
                            .await?,
                        false,
                    )
                }
            },
            None => (
                self.repo
                    .apply_provider_metrics(snapshot_date, None, None)
                    .await?,
                false,
            ),
        };

        Ok(RefreshSnapshotResult {
            snapshot,
            storage_available,
        })
    }

    pub async fn load_history(&self) -> Result<Vec<AdminStatsSnapshot>, sqlx::Error> {
        self.repo.list_snapshots().await
    }
}

pub async fn collect_admin_stats_once(
    service: &AdminStatsService,
    snapshot_date: NaiveDate,
) -> Result<RefreshSnapshotResult, sqlx::Error> {
    service
        .repo
        .finalize_snapshots_before(snapshot_date)
        .await?;
    service
        .repo
        .backfill_historical_snapshots(snapshot_date)
        .await?;
    service.refresh_snapshot(snapshot_date).await
}
