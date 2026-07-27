use memebucket_server::repositories::{
    admin::AdminRepository,
    users::{UserRepo, UserRepository},
};
use sqlx::SqlitePool;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn missing_permissions_are_false_and_grants_are_idempotent() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let user = users
        .upsert_by_provider("discord", "permission-user", None, None)
        .await
        .unwrap();
    let repo = AdminRepository::new(pool);

    let initial = repo
        .search_users(Some("permission-user"), 50)
        .await
        .unwrap();
    assert!(initial[0].permissions.is_empty());

    repo.set_permission(user.id, "upload_local_images", true)
        .await
        .unwrap();
    repo.set_permission(user.id, "upload_local_images", true)
        .await
        .unwrap();
    let granted = repo
        .search_users(Some("permission-user"), 50)
        .await
        .unwrap();
    assert_eq!(granted[0].permissions.len(), 1);

    repo.set_permission(user.id, "upload_local_images", false)
        .await
        .unwrap();
    repo.set_permission(user.id, "upload_local_images", false)
        .await
        .unwrap();
    let revoked = repo
        .search_users(Some("permission-user"), 50)
        .await
        .unwrap();
    assert!(revoked[0].permissions.is_empty());
}

#[tokio::test]
async fn permissions_reject_invalid_names_and_cascade_on_user_delete() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let user = users
        .upsert_by_provider("discord", "cascade-user", None, None)
        .await
        .unwrap();

    let invalid_permission =
        sqlx::query("INSERT INTO user_permissions (user_id, permission) VALUES (?, ?)")
            .bind(user.id.to_string())
            .bind("not_a_permission")
            .execute(&pool)
            .await;
    assert!(invalid_permission.is_err());

    sqlx::query("INSERT INTO user_permissions (user_id, permission) VALUES (?, ?)")
        .bind(user.id.to_string())
        .bind("view_admin_stats")
        .execute(&pool)
        .await
        .unwrap();
    users.delete(user.id).await.unwrap();

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_permissions WHERE user_id = ?")
            .bind(user.id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}
