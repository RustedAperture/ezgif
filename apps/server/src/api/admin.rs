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
    error::AppError,
    repositories::admin::AdminUserRecord,
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
    _admin: AdminUser,
    Query(query): Query<AdminUsersQuery>,
) -> Result<Json<Vec<AdminUserResponse>>, AppError> {
    let users = state
        .admin_repo
        .search_users(query.q.as_deref(), 50)
        .await?;
    let mut response = Vec::with_capacity(users.len());

    for user in users {
        let is_root_admin = state.is_root_admin(user.id).await?;
        response.push(map_user(user, is_root_admin));
    }

    Ok(Json(response))
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
