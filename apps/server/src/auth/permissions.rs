use uuid::Uuid;

use crate::{config::RootAdminConfig, repositories::users::UserRepo};

pub fn normalize_role(role: &str) -> &'static str {
    if role == "admin" { "admin" } else { "user" }
}

pub fn effective_role(stored_role: &str, is_root_admin: bool) -> &'static str {
    if is_root_admin {
        "admin"
    } else {
        normalize_role(stored_role)
    }
}

pub async fn is_root_admin(
    user_repo: &dyn UserRepo,
    config: &RootAdminConfig,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let identities = user_repo.get_identities(user_id).await?;

    Ok(identities.into_iter().any(|identity| {
        config.is_configured_identity(&identity.provider, &identity.provider_user_id)
    }))
}
