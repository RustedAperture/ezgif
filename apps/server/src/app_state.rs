use crate::repositories::{
    admin::AdminRepository,
    buckets::{BucketRepo, BucketRepository},
    cached::{CachedBucketRepository, CachedImageRepository},
    images::{ImageRepo, ImageRepository},
    send_history::{SendHistoryRepo, SendHistoryRepository},
    users::{UserRepo, UserRepository},
};
use crate::services::storage::StorageService;
use crate::{auth::permissions, config::RootAdminConfig};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct GifSearchCacheEntry {
    pub value: serde_json::Value,
    pub expires_at: Instant,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub user_repo: Arc<dyn UserRepo>,
    pub admin_repo: Arc<AdminRepository>,
    pub bucket_repo: Arc<dyn BucketRepo>,
    pub image_repo: Arc<dyn ImageRepo>,
    pub send_history_repo: Arc<dyn SendHistoryRepo>,
    pub static_dir: PathBuf,
    pub session_secret: String,
    app_user_key_secret: String,
    discord_public_key: String,
    discord_bot_token: String,
    pub klipy_api_key: Option<String>,
    pub klipy_api_base_url: String,
    pub gif_search_cache: Arc<Mutex<HashMap<String, GifSearchCacheEntry>>>,
    pub gif_search_cache_ttl: Duration,
    telegram_bot_token: String,
    telegram_bot_username: String,
    pub storage: Option<Arc<StorageService>>,
    root_admin_config: RootAdminConfig,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        let user_repo = Arc::new(UserRepository::new(pool.clone()));
        let admin_repo = Arc::new(AdminRepository::new(pool.clone()));
        let bucket_repo = Arc::new(CachedBucketRepository::new(BucketRepository::new(
            pool.clone(),
        )));
        let image_repo = Arc::new(CachedImageRepository::new(ImageRepository::new(
            pool.clone(),
        )));
        let send_history_repo = Arc::new(SendHistoryRepository::new(pool.clone()));

        Self {
            pool,
            user_repo,
            admin_repo,
            bucket_repo,
            image_repo,
            send_history_repo,
            static_dir: PathBuf::from("apps/web/out"),
            session_secret: String::new(),
            app_user_key_secret: String::new(),
            discord_public_key: String::new(),
            discord_bot_token: String::new(),
            klipy_api_key: None,
            klipy_api_base_url: "https://api.klipy.com".to_string(),
            gif_search_cache: Arc::new(Mutex::new(HashMap::new())),
            gif_search_cache_ttl: Duration::from_secs(60 * 60 * 6),
            telegram_bot_token: String::new(),
            telegram_bot_username: String::new(),
            storage: None,
            root_admin_config: RootAdminConfig::default(),
        }
    }

    pub fn for_tests(pool: SqlitePool) -> Self {
        Self::new(pool)
    }

    pub fn with_static_dir(mut self, dir: PathBuf) -> Self {
        self.static_dir = dir;
        self
    }

    pub fn with_root_admin_config(mut self, config: RootAdminConfig) -> Self {
        self.root_admin_config = config;
        self
    }

    pub async fn is_root_admin(&self, user_id: uuid::Uuid) -> Result<bool, sqlx::Error> {
        permissions::is_root_admin(self.user_repo.as_ref(), &self.root_admin_config, user_id).await
    }

    pub async fn is_configured_root_identity(
        &self,
        user_id: uuid::Uuid,
        provider: &str,
    ) -> Result<bool, sqlx::Error> {
        let identities = self.user_repo.get_identities(user_id).await?;

        Ok(identities.into_iter().any(|identity| {
            identity.provider == provider
                && self
                    .root_admin_config
                    .is_configured_identity(&identity.provider, &identity.provider_user_id)
        }))
    }

    pub fn with_session_secret(mut self, secret: String) -> Self {
        self.session_secret = secret;
        self
    }

    pub fn session_secret(&self) -> &str {
        &self.session_secret
    }

    pub fn with_app_user_key_secret(mut self, secret: String) -> Self {
        self.app_user_key_secret = secret;
        self
    }

    pub fn app_user_key_secret(&self) -> &str {
        &self.app_user_key_secret
    }

    pub fn with_discord_public_key(mut self, discord_public_key: String) -> Self {
        self.discord_public_key = discord_public_key;
        self
    }

    pub fn discord_public_key(&self) -> &str {
        &self.discord_public_key
    }

    pub fn with_discord_bot_token(mut self, discord_bot_token: String) -> Self {
        self.discord_bot_token = discord_bot_token;
        self
    }

    pub fn discord_bot_token(&self) -> &str {
        &self.discord_bot_token
    }

    pub fn with_klipy_api_key(mut self, klipy_api_key: Option<String>) -> Self {
        self.klipy_api_key = klipy_api_key;
        self
    }

    pub fn with_klipy_api_base_url(mut self, klipy_api_base_url: String) -> Self {
        self.klipy_api_base_url = klipy_api_base_url;
        self
    }

    pub fn with_gif_search_cache_ttl(mut self, gif_search_cache_ttl: Duration) -> Self {
        self.gif_search_cache_ttl = gif_search_cache_ttl;
        self
    }

    pub fn with_telegram(mut self, bot_token: String, bot_username: String) -> Self {
        self.telegram_bot_token = bot_token;
        self.telegram_bot_username = bot_username;
        self
    }

    pub fn telegram_bot_token(&self) -> &str {
        &self.telegram_bot_token
    }
    pub fn telegram_bot_username(&self) -> &str {
        &self.telegram_bot_username
    }
    // Bot token format: "<bot_id>:<secret>". The numeric ID is public info.
    pub fn telegram_bot_id(&self) -> &str {
        self.telegram_bot_token.split(':').next().unwrap_or("")
    }

    pub fn with_storage(mut self, storage: Option<StorageService>) -> Self {
        self.storage = storage.map(Arc::new);
        self
    }

    pub fn storage(&self) -> Option<&StorageService> {
        self.storage.as_deref()
    }
}
