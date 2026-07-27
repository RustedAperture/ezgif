use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use memebucket_server::{
    app_state::AppState,
    auth::sessions::{AuthenticatedUser, create_session},
    config::RootAdminConfig,
    repositories::{
        admin::{AdminRepository, AdminUserRecord},
        users::{StoredIdentity, StoredUser, UserRepo, UserRepository},
    },
    router::build_router_for_tests,
};
use sqlx::SqlitePool;
use std::collections::HashSet;
use tower::ServiceExt;
use uuid::Uuid;

const ROOT_PROVIDER_ID: &str = "root-provider-id-0000";
const TEST_SESSION_SECRET: &str = "admin-api-test-session-secret";

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

struct AdminFixture {
    app: Router,
    pool: SqlitePool,
    root: StoredUser,
    normal_admin: StoredUser,
    normal_user: StoredUser,
    target: StoredUser,
    full_provider_id: String,
}

async fn admin_fixture() -> AdminFixture {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let full_provider_id = "search-provider-id-1234".to_string();
    let root = users
        .upsert_by_provider("discord", ROOT_PROVIDER_ID, Some("Root"), None)
        .await
        .unwrap();
    let normal_admin = users
        .upsert_by_provider("discord", "normal-admin", Some("Admin"), None)
        .await
        .unwrap();
    let normal_user = users
        .upsert_by_provider("discord", "normal-user", Some("User"), None)
        .await
        .unwrap();
    let target = users
        .upsert_by_provider("discord", &full_provider_id, Some("Search User"), None)
        .await
        .unwrap();
    users
        .update_username(target.id, "search-user")
        .await
        .unwrap();
    users
        .link_identity(
            target.id,
            "telegram",
            "search-telegram-id-5678",
            Some("Search User"),
            None,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = ?")
        .bind(normal_admin.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let normal_admin = users.get_by_id(normal_admin.id).await.unwrap().unwrap();

    let state = AppState::for_tests(pool.clone())
        .with_root_admin_config(
            RootAdminConfig::parse(&format!("discord:{ROOT_PROVIDER_ID}")).unwrap(),
        )
        .with_session_secret(TEST_SESSION_SECRET.to_string());

    AdminFixture {
        app: build_router_for_tests(state),
        pool,
        root,
        normal_admin,
        normal_user,
        target,
        full_provider_id,
    }
}

async fn get_admin_users(app: &Router, user: &StoredUser, query: &str) -> Response {
    let mut request = Request::builder()
        .uri(format!("/api/admin/users?q={query}"))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: user.role.clone(),
    });
    app.clone().oneshot(request).await.unwrap()
}

async fn get_profile(app: &Router, user: &StoredUser) -> Response {
    let mut request = Request::builder()
        .uri("/api/account")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: user.role.clone(),
    });
    app.clone().oneshot(request).await.unwrap()
}

async fn patch_role(app: &Router, user: &StoredUser, target_id: Uuid, role: &str) -> Response {
    let mut request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/admin/users/{target_id}/role"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({ "role": role }).to_string()))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: user.role.clone(),
    });
    app.clone().oneshot(request).await.unwrap()
}

async fn patch_permission(
    app: &Router,
    user: &StoredUser,
    target_id: Uuid,
    permission: &str,
    enabled: bool,
) -> Response {
    let mut request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/admin/users/{target_id}/permissions"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "permission": permission, "enabled": enabled }).to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: user.role.clone(),
    });
    app.clone().oneshot(request).await.unwrap()
}

async fn json_body(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn unlink_identity(
    app: &Router,
    session_id: Uuid,
    csrf_token: &str,
    provider: &str,
) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/account/identities/{provider}"))
                .header("cookie", format!("session={session_id}"))
                .header("X-CSRF-Token", csrf_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn admin_search_masks_provider_ids_and_finds_username_or_identity() {
    let fixture = admin_fixture().await;

    let username_response = get_admin_users(&fixture.app, &fixture.root, "search-user").await;
    assert_eq!(username_response.status(), StatusCode::OK);

    let response = get_admin_users(&fixture.app, &fixture.root, "discord:search-provider").await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_body(response).await;
    assert_eq!(json[0]["username"], "search-user");
    assert!(!json.to_string().contains(&fixture.full_provider_id));
    assert!(
        json[0]["identities"][0]["masked_id"]
            .as_str()
            .unwrap()
            .ends_with("1234")
    );
    assert_eq!(json[0]["permissions"]["upload_local_images"], false);
    assert_eq!(json[0]["permissions"]["view_admin_stats"], false);
    assert_eq!(json[0]["permissions"]["manage_permissions"], false);
}

#[tokio::test]
async fn only_root_admins_can_change_roles() {
    let fixture = admin_fixture().await;
    assert_eq!(
        patch_role(
            &fixture.app,
            &fixture.normal_admin,
            fixture.target.id,
            "admin"
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        patch_role(&fixture.app, &fixture.root, fixture.target.id, "admin")
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn configured_root_admin_cannot_be_demoted() {
    let fixture = admin_fixture().await;
    assert_eq!(
        patch_role(&fixture.app, &fixture.root, fixture.root.id, "user")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn promoting_a_user_to_admin_grants_all_permissions_idempotently() {
    let fixture = admin_fixture().await;

    assert_eq!(
        patch_role(&fixture.app, &fixture.root, fixture.normal_user.id, "admin")
            .await
            .status(),
        StatusCode::OK
    );

    let promoted =
        json_body(get_admin_users(&fixture.app, &fixture.root, "normal-user").await).await;
    assert_eq!(promoted[0]["role"], "admin");
    assert_eq!(promoted[0]["permissions"]["upload_local_images"], true);
    assert_eq!(promoted[0]["permissions"]["view_admin_stats"], true);
    assert_eq!(promoted[0]["permissions"]["manage_permissions"], true);

    assert_eq!(
        patch_role(&fixture.app, &fixture.root, fixture.normal_user.id, "admin")
            .await
            .status(),
        StatusCode::OK
    );

    let granted_permissions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_permissions WHERE user_id = ?")
            .bind(fixture.normal_user.id.to_string())
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(granted_permissions, 3);
}

#[tokio::test]
async fn admins_can_toggle_upload_and_stats_but_not_manage_permissions() {
    let fixture = admin_fixture().await;
    assert_eq!(
        patch_permission(
            &fixture.app,
            &fixture.normal_admin,
            fixture.target.id,
            "upload_local_images",
            true,
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        patch_permission(
            &fixture.app,
            &fixture.normal_admin,
            fixture.target.id,
            "view_admin_stats",
            true,
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        patch_permission(
            &fixture.app,
            &fixture.normal_admin,
            fixture.target.id,
            "manage_permissions",
            true,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        patch_permission(
            &fixture.app,
            &fixture.root,
            fixture.target.id,
            "manage_permissions",
            true,
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn unknown_roles_and_missing_grants_fail_closed() {
    let fixture = admin_fixture().await;
    sqlx::query("UPDATE users SET role = 'owner' WHERE id = ?")
        .bind(fixture.target.id.to_string())
        .execute(&fixture.pool)
        .await
        .unwrap();
    let profile = json_body(get_profile(&fixture.app, &fixture.target).await).await;
    assert_eq!(profile["role"], "user");
    assert_eq!(profile["is_root_admin"], false);
}

#[tokio::test]
async fn admin_routes_require_authentication_and_admin_role() {
    let fixture = admin_fixture().await;
    let unauthenticated = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        get_admin_users(&fixture.app, &fixture.normal_user, "search-user")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn admin_updates_return_not_found_for_missing_target() {
    let fixture = admin_fixture().await;
    assert_eq!(
        patch_role(&fixture.app, &fixture.root, Uuid::new_v4(), "admin")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        patch_permission(
            &fixture.app,
            &fixture.root,
            Uuid::new_v4(),
            "upload_local_images",
            true,
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn configured_root_identity_cannot_be_unlinked() {
    let fixture = admin_fixture().await;
    let users = UserRepository::new(fixture.pool.clone());
    users
        .link_identity(
            fixture.root.id,
            "telegram",
            "root-secondary-id-5678",
            Some("Root"),
            None,
        )
        .await
        .unwrap();
    let (session_id, csrf_token) =
        create_session(&fixture.pool, fixture.root.id, TEST_SESSION_SECRET)
            .await
            .unwrap();

    let response = unlink_identity(&fixture.app, session_id, &csrf_token, "discord").await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(
        users
            .get_identities(fixture.root.id)
            .await
            .unwrap()
            .iter()
            .any(|identity| {
                identity.provider == "discord" && identity.provider_user_id == ROOT_PROVIDER_ID
            })
    );
}

#[tokio::test]
async fn non_root_users_can_unlink_a_secondary_identity_but_not_the_last_one() {
    let fixture = admin_fixture().await;
    let users = UserRepository::new(fixture.pool.clone());
    users
        .link_identity(
            fixture.normal_user.id,
            "telegram",
            "normal-user-secondary-5678",
            Some("User"),
            None,
        )
        .await
        .unwrap();
    let (session_id, csrf_token) =
        create_session(&fixture.pool, fixture.normal_user.id, TEST_SESSION_SECRET)
            .await
            .unwrap();

    assert_eq!(
        unlink_identity(&fixture.app, session_id, &csrf_token, "discord")
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        users
            .count_identities(fixture.normal_user.id)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        unlink_identity(&fixture.app, session_id, &csrf_token, "telegram")
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        users
            .count_identities(fixture.normal_user.id)
            .await
            .unwrap(),
        1
    );
}
