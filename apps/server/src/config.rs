use std::collections::HashSet;
use std::str::FromStr;
use std::{env, fs, net::SocketAddr, path::PathBuf};

use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

#[derive(Clone, Debug, Default)]
pub struct RootAdminConfig {
    configured_identities: HashSet<String>,
}

impl RootAdminConfig {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let mut configured_identities = HashSet::new();

        for entry in raw.split(',') {
            let entry = entry.trim();
            let (provider, provider_user_id) = entry
                .split_once(':')
                .ok_or_else(|| anyhow::anyhow!("root admin entry must include a provider"))?;
            let provider = provider.trim();
            let provider_user_id = provider_user_id.trim();

            if provider_user_id.is_empty() {
                return Err(anyhow::anyhow!("root admin entry must include an ID"));
            }
            if !matches!(provider, "discord" | "telegram") {
                return Err(anyhow::anyhow!("root admin provider is not supported"));
            }

            configured_identities.insert(format!("{provider}:{provider_user_id}"));
        }

        if configured_identities.is_empty() {
            return Err(anyhow::anyhow!("at least one root admin is required"));
        }

        Ok(Self {
            configured_identities,
        })
    }

    pub fn is_configured_identity(&self, provider: &str, provider_user_id: &str) -> bool {
        self.configured_identities
            .contains(&format!("{provider}:{provider_user_id}"))
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub session_secret: String,
    pub discord_application_id: String,
    pub discord_bot_token: String,
    pub discord_public_key: String,
    pub klipy_api_key: Option<String>,
    pub static_dir: PathBuf,
    pub telegram_bot_token: Option<String>,
    pub telegram_bot_username: Option<String>,
    pub b2_key_id: Option<String>,
    pub b2_app_key: Option<String>,
    pub b2_bucket_name: Option<String>,
    pub b2_endpoint: Option<String>,
    pub cdn_base_url: Option<String>,
    pub root_admin_config: RootAdminConfig,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()?;
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/app.db".to_string());
        let discord_application_id = env::var("DISCORD_APPLICATION_ID").unwrap_or_default();
        let discord_bot_token = env::var("DISCORD_BOT_TOKEN").unwrap_or_default();
        let discord_public_key = env::var("DISCORD_PUBLIC_KEY").unwrap_or_default();
        let session_secret = required_env("SESSION_SECRET")?;
        let root_admin_config = RootAdminConfig::parse(&required_env("ADMIN_PROVIDER_IDS")?)?;
        let klipy_api_key = env::var("KLIPY_API_KEY").ok().filter(|v| !v.is_empty());
        let telegram_bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .ok()
            .filter(|v| !v.is_empty());
        let telegram_bot_username = env::var("TELEGRAM_BOT_USERNAME")
            .ok()
            .filter(|v| !v.is_empty());
        let b2_key_id = env::var("B2_KEY_ID").ok().filter(|v| !v.is_empty());
        let b2_app_key = env::var("B2_APP_KEY").ok().filter(|v| !v.is_empty());
        let b2_bucket_name = env::var("B2_BUCKET_NAME").ok().filter(|v| !v.is_empty());
        let b2_endpoint = env::var("B2_ENDPOINT").ok().filter(|v| !v.is_empty());
        let cdn_base_url = env::var("CDN_BASE_URL").ok().filter(|v| !v.is_empty());
        let static_dir = env::var("STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("apps/web/out"));

        Ok(Self {
            bind_addr,
            database_url,
            session_secret,
            discord_application_id,
            discord_bot_token,
            discord_public_key,
            klipy_api_key,
            static_dir,
            telegram_bot_token,
            telegram_bot_username,
            b2_key_id,
            b2_app_key,
            b2_bucket_name,
            b2_endpoint,
            cdn_base_url,
            root_admin_config,
        })
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required"))
}

pub async fn connect_sqlite_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    ensure_sqlite_parent_dir(database_url)?;
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .foreign_keys(true);
    Ok(SqlitePool::connect_with(options).await?)
}

// Used only for running migrations — FK enforcement is disabled so that the
// table-recreation dance in migration 0014 does not cascade-delete child rows.
pub async fn connect_sqlite_pool_for_migrations(database_url: &str) -> anyhow::Result<SqlitePool> {
    ensure_sqlite_parent_dir(database_url)?;
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .foreign_keys(false);
    Ok(SqlitePool::connect_with(options).await?)
}

fn ensure_sqlite_parent_dir(database_url: &str) -> anyhow::Result<()> {
    let Some(path) = sqlite_file_path(database_url) else {
        return Ok(());
    };

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    let url_path = database_url.strip_prefix("sqlite:")?;
    if url_path == ":memory:" {
        return None;
    }

    let path = url_path
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(url_path)
        .strip_prefix("//")
        .unwrap_or(url_path);

    if path.is_empty() {
        return None;
    }

    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::{Config, RootAdminConfig, connect_sqlite_pool};
    use crate::{app_state::AppState, auth::permissions::normalize_role};
    use sqlx::SqlitePool;
    use std::fs;
    use tokio::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::const_new(());
    static ENV_LOCK: Mutex<()> = Mutex::const_new(());

    #[test]
    fn parses_provider_qualified_root_ids() {
        let config = RootAdminConfig::parse("discord:123,telegram:456").unwrap();

        assert!(config.is_configured_identity("discord", "123"));
        assert!(config.is_configured_identity("telegram", "456"));
    }

    #[test]
    fn rejects_missing_provider_or_id() {
        assert!(RootAdminConfig::parse("discord:").is_err());
        assert!(RootAdminConfig::parse(":123").is_err());
        assert!(RootAdminConfig::parse("matrix:123").is_err());
    }

    #[test]
    fn unknown_roles_normalize_to_user() {
        assert_eq!(normalize_role("admin"), "admin");
        assert_eq!(normalize_role("owner"), "user");
        assert_eq!(normalize_role(""), "user");
    }

    #[tokio::test]
    async fn config_requires_admin_provider_ids() {
        let _env_lock = ENV_LOCK.lock().await;
        let old_session_secret = std::env::var("SESSION_SECRET").ok();
        let old_admin_provider_ids = std::env::var("ADMIN_PROVIDER_IDS").ok();
        unsafe {
            std::env::set_var("SESSION_SECRET", "test-session-secret");
            std::env::remove_var("ADMIN_PROVIDER_IDS");
        }

        let result = Config::from_env();

        match old_session_secret {
            Some(value) => unsafe { std::env::set_var("SESSION_SECRET", value) },
            None => unsafe { std::env::remove_var("SESSION_SECRET") },
        }
        match old_admin_provider_ids {
            Some(value) => unsafe { std::env::set_var("ADMIN_PROVIDER_IDS", value) },
            None => unsafe { std::env::remove_var("ADMIN_PROVIDER_IDS") },
        }

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_app_state_uses_empty_root_admin_configuration() {
        let pool = SqlitePool::connect_lazy("sqlite::memory:").unwrap();

        let _state = AppState::for_tests(pool);
    }

    #[tokio::test]
    async fn config_defaults_to_repo_local_sqlite_path() {
        let _cwd_lock = CWD_LOCK.lock().await;
        let _env_lock = ENV_LOCK.lock().await;
        let old_discord_public_key = std::env::var("DISCORD_PUBLIC_KEY").ok();
        let old_session_secret = std::env::var("SESSION_SECRET").ok();
        let old_admin_provider_ids = std::env::var("ADMIN_PROVIDER_IDS").ok();
        unsafe {
            std::env::remove_var("DISCORD_PUBLIC_KEY");
            std::env::set_var("SESSION_SECRET", "test-session-secret");
            std::env::set_var("ADMIN_PROVIDER_IDS", "discord:123");
        }

        let config = Config::from_env().unwrap();

        match old_discord_public_key {
            Some(value) => unsafe {
                std::env::set_var("DISCORD_PUBLIC_KEY", value);
            },
            None => unsafe {
                std::env::remove_var("DISCORD_PUBLIC_KEY");
            },
        }
        match old_session_secret {
            Some(value) => unsafe {
                std::env::set_var("SESSION_SECRET", value);
            },
            None => unsafe {
                std::env::remove_var("SESSION_SECRET");
            },
        }
        match old_admin_provider_ids {
            Some(value) => unsafe {
                std::env::set_var("ADMIN_PROVIDER_IDS", value);
            },
            None => unsafe {
                std::env::remove_var("ADMIN_PROVIDER_IDS");
            },
        }

        assert_eq!(config.database_url, "sqlite://data/app.db");
        assert_eq!(config.discord_public_key, "");
        assert_eq!(config.session_secret, "test-session-secret");
    }

    #[tokio::test]
    async fn connect_sqlite_pool_creates_missing_parent_dir() {
        let _cwd_lock = CWD_LOCK.lock().await;
        let old_cwd = std::env::current_dir().unwrap();
        let root = std::env::temp_dir().join(format!("random-media-bot-{}", uuid::Uuid::new_v4()));
        let db_path = root.join("data").join("app.db");

        fs::create_dir_all(&root).unwrap();
        std::env::set_current_dir(&root).unwrap();

        assert!(!db_path.parent().unwrap().exists());

        let pool = connect_sqlite_pool("sqlite://data/app.db").await.unwrap();

        assert!(db_path.parent().unwrap().exists());
        assert!(db_path.exists());
        drop(pool);

        std::env::set_current_dir(old_cwd).unwrap();
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
    }
}
