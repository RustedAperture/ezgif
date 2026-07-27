use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    api::ValidatedJson,
    app_state::AppState,
    auth::{middleware::RequireCsrf, permissions::effective_role, sessions::AuthenticatedUser},
    services::account::AccountService,
};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Serialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub is_root_admin: bool,
}

#[derive(Deserialize, Validate)]
pub struct UpdateUsernameRequest {
    #[validate(length(
        min = 3,
        max = 32,
        message = "Username must be between 3 and 32 characters"
    ))]
    #[validate(custom(function = validate_username))]
    pub username: String,
}

fn validate_username(username: &str) -> Result<(), validator::ValidationError> {
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        let mut err = validator::ValidationError::new("invalid_username");
        err.message = Some("Username can only contain letters, numbers, and underscores".into());
        return Err(err);
    }
    Ok(())
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, crate::error::AppError> {
    if let Some(session_id) = crate::auth::sessions::read_session_cookie(&headers) {
        let _ = crate::auth::sessions::delete_session(&state.pool, &session_id).await;
    }

    let mut response_headers = axum::http::HeaderMap::new();
    if let Ok(c) = crate::auth::sessions::expired_session_cookie().parse() {
        response_headers.append(axum::http::header::SET_COOKIE, c);
    }
    if let Ok(c) = crate::auth::sessions::expired_csrf_cookie().parse() {
        response_headers.append(axum::http::header::SET_COOKIE, c);
    }

    Ok((
        StatusCode::OK,
        response_headers,
        Json(serde_json::json!({ "logged_out": true })),
    ))
}

pub async fn export_account(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<crate::services::account::ExportedUserData>, crate::error::AppError> {
    let service = AccountService::new(
        state.user_repo.clone(),
        state.bucket_repo.clone(),
        state.image_repo.clone(),
    );
    Ok(Json(service.export_user_data(user.user_id).await?))
}

pub async fn import_account(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<crate::services::account::ExportedUserData>,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let service = AccountService::new(
        state.user_repo.clone(),
        state.bucket_repo.clone(),
        state.image_repo.clone(),
    );
    let (buckets_created, images_created) = service.import_user_data(user.user_id, request).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "bucketsCreated": buckets_created,
        "imagesCreated": images_created,
    })))
}

pub async fn delete_account(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, crate::error::AppError> {
    let service = AccountService::new(
        state.user_repo.clone(),
        state.bucket_repo.clone(),
        state.image_repo.clone(),
    );
    service.delete_account(user.user_id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn get_profile(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<UserProfileResponse>, crate::error::AppError> {
    let repo = state.user_repo.clone();
    let stored = repo
        .get_by_id(user.user_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound)?;
    let is_root_admin = state.is_root_admin(user.user_id).await?;

    Ok(Json(UserProfileResponse {
        id: stored.id.to_string(),
        username: stored.username,
        display_name: stored.display_name,
        avatar_url: stored.avatar_url,
        role: effective_role(&stored.role, is_root_admin).to_string(),
        is_root_admin,
    }))
}

pub async fn update_username(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    ValidatedJson(req): ValidatedJson<UpdateUsernameRequest>,
) -> Result<Json<UserProfileResponse>, crate::error::AppError> {
    let repo = state.user_repo.clone();

    // SQLite UNIQUE constraint will catch duplicates, but we could return a nice error.
    match repo.update_username(user.user_id, &req.username).await {
        Ok(_) => (),
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            return Err(crate::error::AppError::BadRequest(
                "Username is already taken".into(),
            ));
        }
        Err(e) => return Err(e.into()),
    }

    get_profile(State(state), user).await
}

#[derive(Serialize)]
pub struct IdentityResponse {
    pub provider: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

pub async fn list_identities(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let identities = state.user_repo.get_identities(user.user_id).await?;
    let response: Vec<IdentityResponse> = identities
        .into_iter()
        .map(|i| IdentityResponse {
            provider: i.provider,
            display_name: i.display_name,
            avatar_url: i.avatar_url,
        })
        .collect();
    Ok(Json(response))
}

pub async fn unlink_identity(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    _csrf: RequireCsrf,
    Path(provider): Path<String>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    if state
        .is_configured_root_identity(user.user_id, &provider)
        .await?
    {
        return Err(crate::error::AppError::Forbidden);
    }

    let count = state.user_repo.count_identities(user.user_id).await?;
    if count <= 1 {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Cannot remove your only login method"
            })),
        )
            .into_response());
    }

    state
        .user_repo
        .unlink_identity(user.user_id, &provider)
        .await?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "unlinked": true })),
    )
        .into_response())
}
