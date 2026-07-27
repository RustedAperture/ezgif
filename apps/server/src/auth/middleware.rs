use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    auth::permissions::effective_role,
    auth::sessions::{
        AuthenticatedUser, lookup_session, lookup_session_info, read_session_cookie,
        verify_csrf_token,
    },
};

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Allow tests to inject an AuthenticatedUser directly via extensions
        if let Some(user) = parts.extensions.get::<AuthenticatedUser>() {
            return Ok(user.clone());
        }

        let session_id = read_session_cookie(&parts.headers).ok_or(StatusCode::UNAUTHORIZED)?;

        lookup_session(&state.pool, &session_id)
            .await
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

pub struct OptionalUser(pub Option<AuthenticatedUser>);

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state)
            .await
            .ok();
        Ok(Self(user))
    }
}

pub struct AdminUser {
    pub user_id: Uuid,
    pub is_root_admin: bool,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        let is_root_admin = state
            .is_root_admin(user.user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if effective_role(&user.role, is_root_admin) != "admin" {
            return Err(StatusCode::FORBIDDEN);
        }

        Ok(AdminUser {
            user_id: user.user_id,
            is_root_admin,
        })
    }
}

pub struct RequireCsrf;

impl FromRequestParts<AppState> for RequireCsrf {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session_id = read_session_cookie(&parts.headers).ok_or(StatusCode::UNAUTHORIZED)?;

        let session = lookup_session_info(&state.pool, &session_id)
            .await
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let csrf_token = parts
            .headers
            .get("X-CSRF-Token")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::FORBIDDEN)?;

        if !verify_csrf_token(state.session_secret(), csrf_token, &session.csrf_token_hash) {
            return Err(StatusCode::FORBIDDEN);
        }

        Ok(RequireCsrf)
    }
}
