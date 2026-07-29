ALTER TABLE admin_stats_snapshots
ADD COLUMN daily_send_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE admin_stats_snapshots
ADD COLUMN finalized INTEGER NOT NULL DEFAULT 1 CHECK (finalized IN (0, 1));

UPDATE admin_stats_snapshots
SET finalized = 0
WHERE unique_file_count IS NOT NULL;
