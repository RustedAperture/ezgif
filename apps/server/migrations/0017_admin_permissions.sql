CREATE TABLE user_permissions (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission TEXT NOT NULL CHECK(permission IN (
        'upload_local_images',
        'view_admin_stats',
        'manage_permissions'
    )),
    granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, permission)
);
