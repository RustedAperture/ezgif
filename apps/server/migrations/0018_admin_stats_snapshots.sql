CREATE TABLE admin_stats_snapshots (
    snapshot_date TEXT PRIMARY KEY NOT NULL,
    user_count INTEGER NOT NULL,
    bucket_count INTEGER NOT NULL,
    image_link_count INTEGER NOT NULL,
    unique_file_count INTEGER,
    send_count INTEGER NOT NULL,
    daily_send_count INTEGER NOT NULL,
    b2_object_count INTEGER,
    b2_bytes INTEGER
);
