use memebucket_server::{
    app_state::AppState,
    config::{Config, connect_sqlite_pool, connect_sqlite_pool_for_migrations},
    discord::commands::command_definitions,
    router::build_router,
    services::migration::run_cdn_migration,
    services::storage::StorageService,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;

    tracing::info!("Running database migrations...");
    let migration_pool = connect_sqlite_pool_for_migrations(&config.database_url).await?;
    sqlx::migrate!("./migrations")
        .run(&migration_pool)
        .await
        .expect("database migration failed");
    migration_pool.close().await;
    tracing::info!("Database migrations complete.");

    let pool = connect_sqlite_pool(&config.database_url).await?;

    // Register Discord slash commands asynchronously in the background if credentials are configured
    if !config.discord_application_id.is_empty() && !config.discord_bot_token.is_empty() {
        let app_id = config.discord_application_id.clone();
        let bot_token = config.discord_bot_token.clone();
        tokio::spawn(async move {
            register_discord_commands(&app_id, &bot_token).await;
        });
    }

    let storage = match (
        &config.b2_key_id,
        &config.b2_app_key,
        &config.b2_bucket_name,
        &config.b2_endpoint,
        &config.cdn_base_url,
    ) {
        (Some(key_id), Some(app_key), Some(bucket), Some(endpoint), Some(cdn_url)) => {
            match StorageService::new(bucket, endpoint, key_id, app_key, cdn_url, pool.clone()) {
                Ok(svc) => {
                    tracing::info!("B2 storage configured, CDN: {}", cdn_url);
                    Some(svc)
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize B2 storage: {e}");
                    None
                }
            }
        }
        _ => {
            tracing::warn!("B2 env vars not set — media permanence disabled");
            None
        }
    };

    // Spawn CDN migration job if storage is configured
    if let Some(ref storage_svc) = storage {
        let pool_clone = pool.clone();
        let storage_clone = Arc::new(storage_svc.clone());
        tokio::spawn(async move {
            run_cdn_migration(pool_clone, storage_clone).await;
        });
    }

    let app = build_router(
        AppState::new(pool)
            .with_root_admin_config(config.root_admin_config)
            .with_session_secret(config.session_secret)
            .with_app_user_key_secret(config.app_user_key_secret)
            .with_discord_public_key(config.discord_public_key)
            .with_discord_bot_token(config.discord_bot_token)
            .with_static_dir(config.static_dir.clone())
            .with_klipy_api_key(config.klipy_api_key)
            .with_telegram(
                config.telegram_bot_token.unwrap_or_default(),
                config.telegram_bot_username.unwrap_or_default(),
            )
            .with_storage(storage),
    );
    let listener = TcpListener::bind(config.bind_addr).await?;

    tracing::info!("listening on {}", config.bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn register_discord_commands(application_id: &str, bot_token: &str) {
    let commands = command_definitions();
    let client = reqwest::Client::new();
    let url = format!("https://discord.com/api/v10/applications/{application_id}/commands");

    for command in &commands {
        let name = command["name"].as_str().unwrap_or("unknown");
        match client
            .post(&url)
            .header("Authorization", format!("Bot {bot_token}"))
            .json(command)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("registered Discord command: {name}");
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!("failed to register command {name}: {status} {body}");
            }
            Err(err) => {
                tracing::warn!("error registering command {name}: {err}");
            }
        }
    }
}
