use memebucket_server::repositories::{
    admin::{AdminRepository, AdminUserRecord},
    users::{StoredIdentity, UserRepo, UserRepository},
};
use sqlx::SqlitePool;
use std::collections::HashSet;
use uuid::Uuid;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[test]
fn admin_user_debug_output_redacts_provider_ids() {
    let provider_user_id = "discord-provider-id-9876543210";
    let record = AdminUserRecord {
        id: Uuid::new_v4(),
        username: Some("admin".to_string()),
        display_name: Some("Admin".to_string()),
        role: "admin".to_string(),
        identities: vec![StoredIdentity {
            id: Uuid::new_v4(),
            provider: "discord".to_string(),
            provider_user_id: provider_user_id.to_string(),
            display_name: None,
            avatar_url: None,
        }],
        permissions: HashSet::new(),
    };

    let debug_output = format!("{record:?}");

    assert!(!debug_output.contains(provider_user_id));
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
