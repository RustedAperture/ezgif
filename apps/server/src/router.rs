use axum::{
    Router,
    extract::DefaultBodyLimit,
    response::IntoResponse,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};

use crate::{
    api::{
        account::{
            delete_account, export_account, get_profile, import_account, list_identities, logout,
            unlink_identity, update_username,
        },
        admin::{list_users, update_permission, update_role},
        buckets::{create_bucket, delete_bucket, list_buckets, rename_bucket},
        gifs::search_gifs,
        images::{
            create_image, delete_image, list_images, record_image_send, search_images,
            update_image, upload_image,
        },
    },
    app_state::AppState,
    auth::discord_oauth::{handle_discord_oauth_callback, start_discord_oauth},
    auth::telegram_oauth::{get_telegram_config, handle_telegram_callback},
    discord::interactions::handle_interaction,
    static_files::static_fallback,
};

const LOCAL_IMAGE_UPLOAD_BODY_LIMIT_BYTES: usize = 21 * 1024 * 1024;

pub fn build_router(state: AppState) -> Router {
    build_router_internal(state, false)
}

pub fn build_router_for_tests(state: AppState) -> Router {
    build_router_internal(state, true)
}

fn build_router_internal(state: AppState, is_test: bool) -> Router {
    let global_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(20)
            .burst_size(100)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .unwrap(),
    );
    let strict_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(10)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .unwrap(),
    );

    let mut api_routes = Router::new()
        .route("/health", get(health))
        .route("/api/auth/logout", post(logout))
        .route("/api/gifs/search", get(search_gifs))
        .route("/api/images/search", get(search_images))
        .route(
            "/api/buckets/{bucket_id}/images",
            get(list_images).post(create_image),
        )
        .route(
            "/api/buckets/{bucket_id}/images/upload",
            post(upload_image).layer(DefaultBodyLimit::max(LOCAL_IMAGE_UPLOAD_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/buckets/{bucket_id}/images/bulk",
            axum::routing::patch(crate::api::images::bulk_update_images)
                .delete(crate::api::images::bulk_delete_images),
        )
        .route(
            "/api/buckets/{bucket_id}/images/bulk/move",
            post(crate::api::images::bulk_move_images),
        )
        .route(
            "/api/buckets/{bucket_id}/images/{image_id}",
            delete(delete_image).patch(update_image),
        )
        .route(
            "/api/buckets/{bucket_id}/images/{image_id}/move",
            post(crate::api::images::move_image),
        )
        .route(
            "/api/buckets/{bucket_id}/images/{image_id}/send",
            post(record_image_send),
        )
        .route(
            "/api/buckets/{bucket_id}",
            delete(delete_bucket).patch(rename_bucket),
        )
        .route(
            "/api/buckets/{bucket_id}/share",
            post(crate::api::buckets::share_bucket),
        )
        .route(
            "/api/buckets/{bucket_id}/unshare",
            post(crate::api::buckets::unshare_bucket),
        )
        .route(
            "/api/share/{token}",
            get(crate::api::buckets::get_shared_bucket),
        )
        .route(
            "/api/share/{token}/subscribe",
            post(crate::api::buckets::subscribe_bucket),
        )
        .route(
            "/api/buckets/{bucket_id}/unsubscribe",
            post(crate::api::buckets::unsubscribe_bucket),
        )
        .route(
            "/api/buckets/{bucket_id}/whitelist-enabled",
            axum::routing::patch(crate::api::buckets::set_whitelist_enabled),
        )
        .route(
            "/api/buckets/{bucket_id}/whitelist",
            get(crate::api::buckets::list_whitelist_users)
                .post(crate::api::buckets::add_whitelist_user),
        )
        .route(
            "/api/buckets/{bucket_id}/whitelist/{username}",
            delete(crate::api::buckets::remove_whitelist_user),
        )
        .route("/api/account/export", get(export_account))
        .route("/api/account/import", post(import_account))
        .route("/api/account", get(get_profile).delete(delete_account))
        .route(
            "/api/account/username",
            axum::routing::patch(update_username),
        )
        .route("/api/account/identities", get(list_identities))
        .route(
            "/api/account/identities/{provider}",
            delete(unlink_identity),
        )
        .route("/api/admin/users", get(list_users))
        .route(
            "/api/admin/users/{user_id}/role",
            axum::routing::patch(update_role),
        )
        .route(
            "/api/admin/users/{user_id}/permissions",
            axum::routing::patch(update_permission),
        );

    if !is_test {
        api_routes = api_routes
            .route(
                "/auth/discord/start",
                get(start_discord_oauth).layer(GovernorLayer::new(strict_governor_conf.clone())),
            )
            .route(
                "/auth/discord/callback",
                get(handle_discord_oauth_callback)
                    .layer(GovernorLayer::new(strict_governor_conf.clone())),
            )
            .route(
                "/auth/telegram/callback",
                get(handle_telegram_callback)
                    .layer(GovernorLayer::new(strict_governor_conf.clone())),
            )
            .route("/api/telegram/config", get(get_telegram_config))
            .route("/api/buckets", get(list_buckets).post(create_bucket))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                csrf_middleware,
            ))
            .layer(GovernorLayer::new(global_governor_conf.clone()));
    } else {
        api_routes = api_routes
            .route("/auth/discord/start", get(start_discord_oauth))
            .route("/auth/discord/callback", get(handle_discord_oauth_callback))
            .route("/auth/telegram/callback", get(handle_telegram_callback))
            .route("/api/telegram/config", get(get_telegram_config))
            .route("/api/buckets", get(list_buckets).post(create_bucket));
    }

    Router::new()
        .merge(api_routes)
        .route("/discord/interactions", post(handle_interaction))
        .fallback(static_fallback)
        .with_state(state)
}

async fn csrf_middleware(
    state: axum::extract::State<AppState>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method();
    if method == axum::http::Method::POST
        || method == axum::http::Method::PUT
        || method == axum::http::Method::PATCH
        || method == axum::http::Method::DELETE
    {
        use axum::extract::FromRequestParts;
        let (mut parts, body) = req.into_parts();
        match crate::auth::middleware::RequireCsrf::from_request_parts(&mut parts, &state).await {
            Ok(_) => {
                req = axum::extract::Request::from_parts(parts, body);
            }
            Err(status) => return status.into_response(),
        }
    }
    next.run(req).await
}

async fn health() -> &'static str {
    "ok"
}
