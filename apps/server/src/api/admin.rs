use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    auth::{middleware::AdminUser, permissions::effective_role},
    domain::user_key::DiscordUserKey,
    error::AppError,
    repositories::{admin::AdminUserRecord, admin_stats::AdminStatsSnapshot},
};

const UPLOAD_LOCAL_IMAGES: &str = "upload_local_images";
const VIEW_ADMIN_STATS: &str = "view_admin_stats";
const MANAGE_PERMISSIONS: &str = "manage_permissions";

#[derive(Deserialize)]
pub struct AdminUsersQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

#[derive(Deserialize)]
pub struct UpdatePermissionRequest {
    pub permission: String,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct AdminIdentityResponse {
    pub provider: String,
    pub masked_id: String,
}

#[derive(Serialize)]
pub struct AdminPermissionsResponse {
    pub upload_local_images: bool,
    pub view_admin_stats: bool,
    pub manage_permissions: bool,
}

#[derive(Serialize)]
pub struct AdminUserResponse {
    pub id: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub role: String,
    pub is_root_admin: bool,
    pub identities: Vec<AdminIdentityResponse>,
    pub permissions: AdminPermissionsResponse,
}

#[derive(Serialize)]
pub struct AdminStatsSnapshotResponse {
    pub snapshot_date: String,
    pub user_count: i64,
    pub bucket_count: i64,
    pub image_link_count: i64,
    pub unique_file_count: Option<i64>,
    pub send_count: i64,
    pub daily_send_count: i64,
    pub b2_object_count: Option<i64>,
    pub b2_bytes: Option<i64>,
}

#[derive(Serialize)]
pub struct AdminStatsStorageResponse {
    pub configured: bool,
    pub available: bool,
    pub first_complete_history_date: Option<String>,
}

#[derive(Serialize)]
pub struct AdminStatsResponse {
    pub current: AdminStatsSnapshotResponse,
    pub history: Vec<AdminStatsSnapshotResponse>,
    pub storage: AdminStatsStorageResponse,
}

pub fn mask_provider_user_id(provider: &str, provider_user_id: &str) -> String {
    let suffix = provider_user_id
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    if provider_user_id.chars().count() <= 4 {
        format!("{provider}:****")
    } else {
        format!("{provider}:****{suffix}")
    }
}

pub async fn list_users(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<AdminUsersQuery>,
) -> Result<Json<Vec<AdminUserResponse>>, AppError> {
    if !admin.is_root_admin
        && !state
            .admin_repo
            .has_permission(admin.user_id, MANAGE_PERMISSIONS)
            .await?
    {
        return Err(AppError::Forbidden);
    }

    let discord_search_key = query
        .q
        .as_deref()
        .and_then(|value| raw_discord_search_key(value, state.app_user_key_secret()));
    let users = state
        .admin_repo
        .search_users_with_discord_key(query.q.as_deref(), discord_search_key.as_deref(), 50)
        .await?;
    let mut response = Vec::with_capacity(users.len());

    for user in users {
        let is_root_admin = state.is_root_admin(user.id).await?;
        response.push(map_user(user, is_root_admin));
    }

    Ok(Json(response))
}

fn raw_discord_search_key(query: &str, secret: &str) -> Option<String> {
    let query = query.trim();
    let raw_id = query.strip_prefix("discord:").unwrap_or(query);
    let is_discord_query = query.starts_with("discord:")
        || (!raw_id.is_empty() && raw_id.chars().all(|character| character.is_ascii_digit()));

    if !is_discord_query || secret.is_empty() {
        return None;
    }

    Some(
        DiscordUserKey::derive(secret.as_bytes(), raw_id)
            .as_hex()
            .to_string(),
    )
}

pub async fn update_role(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateRoleRequest>,
) -> Result<StatusCode, AppError> {
    if !matches!(request.role.as_str(), "user" | "admin") {
        return Err(AppError::BadRequest(
            "role must be user or admin".to_string(),
        ));
    }
    if !admin.is_root_admin {
        return Err(AppError::Forbidden);
    }
    if state.user_repo.get_by_id(user_id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    if request.role == "user" && state.is_root_admin(user_id).await? {
        return Err(AppError::Forbidden);
    }

    state.admin_repo.set_role(user_id, &request.role).await?;
    Ok(StatusCode::OK)
}

pub async fn get_stats(
    State(state): State<AppState>,
    admin: AdminUser,
) -> Result<Json<AdminStatsResponse>, AppError> {
    if !admin.is_root_admin
        && !state
            .admin_repo
            .has_permission(admin.user_id, VIEW_ADMIN_STATS)
            .await?
    {
        return Err(AppError::Forbidden);
    }

    let mut history = state.admin_stats_service().load_history().await?;
    history.sort_by_key(|snapshot| snapshot.snapshot_date);
    let current = history
        .last()
        .cloned()
        .ok_or_else(|| AppError::InternalServerError("admin stats unavailable".to_string()))?;
    let storage_configured = state.storage().is_some();

    let first_complete_history_date = history
        .iter()
        .find(|snapshot| has_complete_storage_history(snapshot))
        .map(|snapshot| snapshot.snapshot_date.format("%Y-%m-%d").to_string());

    Ok(Json(AdminStatsResponse {
        current: map_stats_snapshot(current.clone()),
        history: history.into_iter().map(map_stats_snapshot).collect(),
        storage: AdminStatsStorageResponse {
            configured: storage_configured,
            available: storage_configured && has_complete_storage_history(&current),
            first_complete_history_date,
        },
    }))
}

pub async fn update_permission(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdatePermissionRequest>,
) -> Result<StatusCode, AppError> {
    if !matches!(
        request.permission.as_str(),
        UPLOAD_LOCAL_IMAGES | VIEW_ADMIN_STATS | MANAGE_PERMISSIONS
    ) {
        return Err(AppError::BadRequest("invalid permission".to_string()));
    }
    if request.permission == MANAGE_PERMISSIONS && !admin.is_root_admin {
        return Err(AppError::Forbidden);
    }
    if state.user_repo.get_by_id(user_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    state
        .admin_repo
        .set_permission(user_id, &request.permission, request.enabled)
        .await?;
    Ok(StatusCode::OK)
}

fn has_complete_storage_history(snapshot: &AdminStatsSnapshot) -> bool {
    snapshot.unique_file_count.is_some()
        && snapshot.b2_object_count.is_some()
        && snapshot.b2_bytes.is_some()
}

fn map_user(user: AdminUserRecord, is_root_admin: bool) -> AdminUserResponse {
    AdminUserResponse {
        id: user.id.to_string(),
        username: user.username,
        display_name: user.display_name,
        role: effective_role(&user.role, is_root_admin).to_string(),
        is_root_admin,
        identities: user
            .identities
            .into_iter()
            .map(|identity| AdminIdentityResponse {
                masked_id: mask_provider_user_id(&identity.provider, &identity.provider_user_id),
                provider: identity.provider,
            })
            .collect(),
        permissions: AdminPermissionsResponse {
            upload_local_images: user.permissions.contains(UPLOAD_LOCAL_IMAGES),
            view_admin_stats: user.permissions.contains(VIEW_ADMIN_STATS),
            manage_permissions: user.permissions.contains(MANAGE_PERMISSIONS),
        },
    }
}

fn map_stats_snapshot(snapshot: AdminStatsSnapshot) -> AdminStatsSnapshotResponse {
    AdminStatsSnapshotResponse {
        snapshot_date: snapshot.snapshot_date.format("%Y-%m-%d").to_string(),
        user_count: snapshot.user_count,
        bucket_count: snapshot.bucket_count,
        image_link_count: snapshot.image_link_count,
        unique_file_count: snapshot.unique_file_count,
        send_count: snapshot.send_count,
        daily_send_count: snapshot.daily_send_count,
        b2_object_count: snapshot.b2_object_count,
        b2_bytes: snapshot.b2_bytes,
    }
}
