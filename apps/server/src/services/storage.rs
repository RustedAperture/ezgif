use object_store::{
    Attribute, AttributeValue, Attributes, ObjectStore, PutOptions, aws::AmazonS3Builder,
    path::Path as ObjPath,
};
use sqlx::SqlitePool;
use std::io::Cursor;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("fetch failed: {0}")]
    FetchFailed(String),
    #[error("invalid image data")]
    InvalidImage,
    #[error("upload failed: {0}")]
    UploadFailed(String),
}

#[derive(Clone)]
pub struct StorageService {
    store: Arc<dyn ObjectStore>,
    cdn_base_url: String,
    pool: SqlitePool,
}

impl StorageService {
    pub fn new(
        bucket_name: &str,
        endpoint: &str,
        key_id: &str,
        app_key: &str,
        cdn_base_url: &str,
        pool: SqlitePool,
    ) -> anyhow::Result<Self> {
        let region = endpoint
            .trim_end_matches(".backblazeb2.com")
            .strip_prefix("s3.")
            .unwrap_or("us-west-004")
            .to_string();
        let store = AmazonS3Builder::new()
            .with_bucket_name(bucket_name)
            .with_endpoint(format!("https://{}", endpoint))
            .with_region(region)
            .with_access_key_id(key_id)
            .with_secret_access_key(app_key)
            .with_virtual_hosted_style_request(false)
            .build()?;
        Ok(Self {
            store: Arc::new(store),
            cdn_base_url: cdn_base_url.trim_end_matches('/').to_string(),
            pool,
        })
    }

    pub fn new_with_store(
        store: Arc<dyn ObjectStore>,
        cdn_base_url: &str,
        pool: SqlitePool,
    ) -> Self {
        Self {
            store,
            cdn_base_url: cdn_base_url.trim_end_matches('/').to_string(),
            pool,
        }
    }

    pub fn is_discord_cdn(url: &str) -> bool {
        url_host_matches(url, &["cdn.discordapp.com", "media.discordapp.net"])
    }

    pub fn is_twitter_media(url: &str) -> bool {
        url_host_matches(url, &["pbs.twimg.com", "video.twimg.com"])
    }

    pub fn is_bluesky_media(url: &str) -> bool {
        url_host_matches(url, &["cdn.bsky.app"])
    }

    /// Fetches `url` and re-hosts it on B2. Goes through the same SSRF-safe
    /// resolver used for user-submitted image URLs (DNS-pinned to a validated
    /// non-internal IP, redirects re-validated) since this is also reached
    /// from the CDN migration job for URLs that were never fetched at submit
    /// time (e.g. imported account data).
    pub async fn upload_from_url(&self, url: &str) -> Result<String, StorageError> {
        let response = super::images::fetch_success(url)
            .await
            .map_err(|e| StorageError::FetchFailed(e.user_message().to_string()))?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let bytes = response
            .bytes()
            .await
            .map_err(|e| StorageError::FetchFailed(e.to_string()))?;

        let url_for_log = url.to_string();
        let (final_bytes, ext) = if content_type.contains("image/png")
            || content_type.contains("image/jpeg")
        {
            let bytes_clone = bytes.clone();
            match tokio::task::spawn_blocking(move || convert_to_webp(&bytes_clone)).await {
                Ok(Ok(webp_bytes)) => (webp_bytes, "webp"),
                Ok(Err(e)) => {
                    tracing::warn!(
                        "WebP conversion failed for {}, storing original format: {e}",
                        url_for_log
                    );
                    (bytes.to_vec(), ext_from_content_type(&content_type))
                }
                Err(e) => {
                    tracing::warn!(
                        "WebP conversion task panicked for {}, storing original format: {e}",
                        url_for_log
                    );
                    (bytes.to_vec(), ext_from_content_type(&content_type))
                }
            }
        } else if content_type.contains("image/gif") {
            let bytes_clone = bytes.clone();
            match tokio::task::spawn_blocking(move || convert_gif_to_animated_webp(&bytes_clone))
                .await
            {
                Ok(Ok(webp_bytes)) => (webp_bytes, "webp"),
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Animated WebP conversion failed for {}, storing original GIF: {e}",
                        url_for_log
                    );
                    (bytes.to_vec(), "gif")
                }
                Err(e) => {
                    tracing::warn!(
                        "Animated WebP conversion task panicked for {}, storing original GIF: {e}",
                        url_for_log
                    );
                    (bytes.to_vec(), "gif")
                }
            }
        } else if content_type.contains("video/mp4") || content_type.contains("video/webm") {
            match super::video_converter::convert_video_bytes_to_webp(&bytes).await {
                Ok(webp_bytes) => (webp_bytes, "webp"),
                Err(e) => {
                    tracing::warn!(
                        "Video→WebP conversion failed for {}, storing original: {e}",
                        url_for_log
                    );
                    (bytes.to_vec(), ext_from_content_type(&content_type))
                }
            }
        } else {
            (bytes.to_vec(), ext_from_content_type(&content_type))
        };

        self.store_bytes(final_bytes, ext).await
    }

    pub async fn upload_image_bytes(&self, bytes: Vec<u8>) -> Result<String, StorageError> {
        let format = image::guess_format(&bytes).map_err(|_| StorageError::InvalidImage)?;
        let bytes_for_conversion = bytes.clone();
        let webp_bytes = tokio::task::spawn_blocking(move || match format {
            image::ImageFormat::Gif => convert_gif_to_animated_webp(&bytes_for_conversion),
            _ => convert_to_webp(&bytes_for_conversion),
        })
        .await
        .map_err(|e| StorageError::UploadFailed(format!("image conversion task failed: {e}")))?
        .map_err(|_| StorageError::InvalidImage)?;

        self.store_bytes(webp_bytes, "webp").await
    }

    /// Upload raw bytes to B2, deduplicating by content hash.
    /// `source_url` is unused for keying — kept for API compatibility.
    pub async fn upload_bytes(
        &self,
        _source_url: &str,
        bytes: Vec<u8>,
        ext: &str,
    ) -> Result<String, StorageError> {
        self.store_bytes(bytes, ext).await
    }

    /// Deletes deduplicated B2 objects that no image record references anymore.
    /// Database and object-store failures are returned so callers can retry later.
    pub async fn garbage_collect_orphaned_objects(&self) -> Result<usize, StorageError> {
        let orphaned = sqlx::query_as::<_, (String, String)>(
            "SELECT content_hash, cdn_url
             FROM cdn_objects
             WHERE NOT EXISTS (
                 SELECT 1 FROM images WHERE images.cdn_url = cdn_objects.cdn_url
             )",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            StorageError::UploadFailed(format!("B2 garbage-collection query failed: {e}"))
        })?;

        let prefix = format!("{}/", self.cdn_base_url);
        let mut deleted = 0;
        for (content_hash, cdn_url) in orphaned {
            let Some(key) = cdn_url.strip_prefix(&prefix) else {
                tracing::warn!(cdn_url, "Skipping orphaned CDN object with unexpected URL");
                continue;
            };

            self.store.delete(&ObjPath::from(key)).await.map_err(|e| {
                StorageError::UploadFailed(format!("B2 object deletion failed: {e}"))
            })?;
            sqlx::query("DELETE FROM cdn_objects WHERE content_hash = ?")
                .bind(&content_hash)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    StorageError::UploadFailed(format!("B2 garbage-collection cleanup failed: {e}"))
                })?;
            tracing::info!(
                content_hash = %content_hash,
                object_key = %key,
                cdn_url = %cdn_url,
                "Deleted orphaned B2 media object"
            );
            deleted += 1;
        }

        Ok(deleted)
    }

    /// Core upload path: hash bytes, check cdn_objects, upload on miss.
    async fn store_bytes(&self, bytes: Vec<u8>, ext: &str) -> Result<String, StorageError> {
        let content_hash = blake3_hash_bytes(&bytes);

        // Dedup check — non-fatal on DB error
        match lookup_cdn_object(&self.pool, &content_hash).await {
            Ok(Some(cdn_url)) => {
                tracing::debug!("Dedup hit for hash {content_hash}, reusing {cdn_url}");
                return Ok(cdn_url);
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("cdn_objects lookup failed, falling through to upload: {e}");
            }
        }

        let key = ObjPath::from(format!("{}.{}", content_hash, ext));
        let cdn_url = format!("{}/{}", self.cdn_base_url, key);

        tracing::debug!(
            "B2 upload attempt: key={key}, size={} bytes, content_type={}",
            bytes.len(),
            mime_for_ext(ext)
        );

        let attributes = Attributes::from_iter([(
            Attribute::ContentType,
            AttributeValue::from(mime_for_ext(ext)),
        )]);
        let result = self
            .store
            .put_opts(
                &key,
                bytes.into(),
                PutOptions {
                    attributes,
                    ..Default::default()
                },
            )
            .await;

        match &result {
            Ok(_) => tracing::debug!("B2 upload succeeded: key={key}"),
            Err(e) => tracing::error!("B2 upload error (put_opts): {e}"),
        }

        result.map_err(|e| StorageError::UploadFailed(e.to_string()))?;

        if let Err(e) = insert_cdn_object(&self.pool, &content_hash, &cdn_url).await {
            tracing::warn!("cdn_objects insert failed (non-fatal): {e}");
        }

        Ok(cdn_url)
    }
}

/// Compares the URL's actual host against an allowlist — never substring-match
/// the whole URL, since query strings/paths are attacker-controlled and would
/// let e.g. `https://attacker.example/?x=cdn.discordapp.com` pass as trusted.
fn url_host_matches(url: &str, allowed_hosts: &[&str]) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| allowed_hosts.iter().any(|allowed| host == *allowed))
}

pub(crate) fn blake3_hash_bytes(bytes: &[u8]) -> String {
    hex::encode(blake3::hash(bytes).as_bytes())
}

async fn lookup_cdn_object(
    pool: &SqlitePool,
    content_hash: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT cdn_url FROM cdn_objects WHERE content_hash = ?")
        .bind(content_hash)
        .fetch_optional(pool)
        .await
}

async fn insert_cdn_object(
    pool: &SqlitePool,
    content_hash: &str,
    cdn_url: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO cdn_objects (content_hash, cdn_url) VALUES (?, ?)")
        .bind(content_hash)
        .bind(cdn_url)
        .execute(pool)
        .await
        .map(|_| ())
}

fn convert_to_webp(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let img = image::load_from_memory(bytes)?;
    let mut output = Vec::new();
    img.write_to(&mut Cursor::new(&mut output), image::ImageFormat::WebP)?;
    Ok(output)
}

fn convert_gif_to_animated_webp(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use image::AnimationDecoder;
    use image::codecs::gif::GifDecoder;

    let decoder = GifDecoder::new(Cursor::new(bytes))?;
    let frames = decoder.into_frames().collect_frames()?;

    if frames.is_empty() {
        anyhow::bail!("GIF has no frames");
    }

    let (width, height) = frames[0].buffer().dimensions();

    let mut frame_data: Vec<(Vec<u8>, i32)> = Vec::with_capacity(frames.len());
    let mut timestamp_ms: i32 = 0;
    for frame in &frames {
        let (numer, denom) = frame.delay().numer_denom_ms();
        let frame_ms = numer.checked_div(denom).unwrap_or(100) as i32;
        frame_data.push((frame.buffer().as_raw().clone(), timestamp_ms));
        timestamp_ms += frame_ms;
    }

    let config =
        webp::WebPConfig::new().map_err(|_| anyhow::anyhow!("Failed to initialize WebPConfig"))?;
    let mut encoder = webp::AnimEncoder::new(width, height, &config);

    for (rgba, ts) in &frame_data {
        encoder.add_frame(webp::AnimFrame::from_rgba(rgba, width, height, *ts));
    }

    let webp_mem = encoder
        .try_encode()
        .map_err(|e| anyhow::anyhow!("Animated WebP encode failed: {:?}", e))?;
    Ok(webp_mem.to_vec())
}

fn ext_from_content_type(ct: &str) -> &'static str {
    if ct.contains("image/gif") {
        "gif"
    } else if ct.contains("image/webp") {
        "webp"
    } else if ct.contains("image/png") {
        "png"
    } else if ct.contains("image/jpeg") {
        "jpg"
    } else if ct.contains("video/mp4") {
        "mp4"
    } else if ct.contains("video/webm") {
        "webm"
    } else {
        "bin"
    }
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "webp" => "image/webp",
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_discord_cdn_detects_both_hosts() {
        assert!(StorageService::is_discord_cdn(
            "https://cdn.discordapp.com/attachments/123/456/image.png"
        ));
        assert!(StorageService::is_discord_cdn(
            "https://media.discordapp.net/attachments/123/456/image.gif"
        ));
        assert!(!StorageService::is_discord_cdn(
            "https://example.com/image.png"
        ));
        assert!(!StorageService::is_discord_cdn(
            "https://media.memebucket.app/abc123.webp"
        ));
    }

    #[test]
    fn is_discord_cdn_rejects_substring_spoofing_via_query_string() {
        // Regression test: an attacker-controlled host must not be treated as
        // Discord's CDN just because the string "cdn.discordapp.com" appears
        // somewhere in the URL (e.g. a query parameter).
        assert!(!StorageService::is_discord_cdn(
            "https://attacker.example/x?ref=cdn.discordapp.com"
        ));
        assert!(!StorageService::is_discord_cdn(
            "https://cdn.discordapp.com.attacker.example/x"
        ));
    }

    #[test]
    fn is_twitter_media_detects_both_hosts() {
        assert!(StorageService::is_twitter_media(
            "https://pbs.twimg.com/media/abc123.jpg:large"
        ));
        assert!(StorageService::is_twitter_media(
            "https://video.twimg.com/tweet_video/abc123.mp4"
        ));
        assert!(!StorageService::is_twitter_media(
            "https://example.com/image.png"
        ));
        assert!(!StorageService::is_twitter_media(
            "https://media.memebucket.app/abc123.webp"
        ));
    }

    #[test]
    fn is_twitter_media_rejects_substring_spoofing_via_query_string() {
        assert!(!StorageService::is_twitter_media(
            "https://attacker.example/x?ref=pbs.twimg.com"
        ));
    }

    #[test]
    fn is_bluesky_media_detects_bluesky_cdn_images_only() {
        assert!(StorageService::is_bluesky_media(
            "https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:abc/bafkreiabc"
        ));
        assert!(!StorageService::is_bluesky_media(
            "https://video.bsky.app/hls/playlist.m3u8"
        ));
        assert!(!StorageService::is_bluesky_media(
            "https://media.memebucket.app/abc123.webp"
        ));
    }

    #[test]
    fn ext_from_content_type_maps_known_types() {
        assert_eq!(ext_from_content_type("image/gif"), "gif");
        assert_eq!(ext_from_content_type("image/png"), "png");
        assert_eq!(ext_from_content_type("image/jpeg"), "jpg");
        assert_eq!(ext_from_content_type("video/mp4"), "mp4");
        assert_eq!(ext_from_content_type("application/octet-stream"), "bin");
    }

    #[test]
    fn mime_for_ext_maps_all_supported_extensions() {
        assert_eq!(mime_for_ext("webp"), "image/webp");
        assert_eq!(mime_for_ext("png"), "image/png");
        assert_eq!(mime_for_ext("jpg"), "image/jpeg");
        assert_eq!(mime_for_ext("gif"), "image/gif");
        assert_eq!(mime_for_ext("mp4"), "video/mp4");
        assert_eq!(mime_for_ext("webm"), "video/webm");
        assert_eq!(mime_for_ext("bin"), "application/octet-stream");
        assert_eq!(mime_for_ext("unknown"), "application/octet-stream");
    }
}

#[cfg(test)]
mod dedup_tests {
    use super::*;
    use object_store::memory::InMemory;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE cdn_objects (
                content_hash TEXT PRIMARY KEY NOT NULL,
                cdn_url TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE images (
                id TEXT PRIMARY KEY NOT NULL,
                cdn_url TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn sample_png_bytes() -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[tokio::test]
    async fn lookup_cdn_object_returns_none_when_empty() {
        let pool = setup_test_db().await;
        let result = lookup_cdn_object(&pool, "abc123").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn lookup_cdn_object_returns_url_after_insert() {
        let pool = setup_test_db().await;
        insert_cdn_object(&pool, "abc123", "https://cdn.example.com/abc123.webp")
            .await
            .unwrap();
        let result = lookup_cdn_object(&pool, "abc123").await.unwrap();
        assert_eq!(
            result.as_deref(),
            Some("https://cdn.example.com/abc123.webp")
        );
    }

    #[tokio::test]
    async fn insert_cdn_object_is_idempotent() {
        let pool = setup_test_db().await;
        insert_cdn_object(&pool, "abc123", "https://cdn.example.com/abc123.webp")
            .await
            .unwrap();
        // Second insert with same hash must not error (INSERT OR IGNORE)
        let result =
            insert_cdn_object(&pool, "abc123", "https://cdn.example.com/abc123.webp").await;
        assert!(result.is_ok());
    }

    #[test]
    fn blake3_hash_bytes_returns_hex_string() {
        let hash = blake3_hash_bytes(b"hello world");
        // BLAKE3 of "hello world" is deterministic
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn upload_image_bytes_converts_png_to_webp() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let source_bytes = sample_png_bytes();
        let result = svc.upload_image_bytes(source_bytes).await.unwrap();

        assert!(result.ends_with(".webp"));

        let key = result
            .strip_prefix("https://cdn.example.com/")
            .expect("cdn base URL prefix")
            .to_string();
        let stored = store
            .get(&object_store::path::Path::from(key))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(
            image::guess_format(&stored).unwrap(),
            image::ImageFormat::WebP
        );
        image::load_from_memory_with_format(&stored, image::ImageFormat::WebP).unwrap();
    }

    #[tokio::test]
    async fn upload_image_bytes_reuses_existing_webp_object_for_duplicate_source() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let source_bytes = sample_png_bytes();

        let first = svc.upload_image_bytes(source_bytes.clone()).await.unwrap();
        let second = svc.upload_image_bytes(source_bytes).await.unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn upload_image_bytes_returns_typed_invalid_image_error_for_malformed_bytes() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let error = svc
            .upload_image_bytes(b"definitely-not-a-real-image".to_vec())
            .await
            .unwrap_err();

        assert!(matches!(error, StorageError::InvalidImage));
    }

    #[tokio::test]
    async fn store_bytes_on_miss_uploads_and_inserts() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let bytes = b"hello world".to_vec();
        let hash = blake3_hash_bytes(&bytes);
        let result = svc.store_bytes(bytes, "webp").await.unwrap();

        // Correct CDN URL returned
        assert_eq!(result, format!("https://cdn.example.com/{hash}.webp"));

        // Object was uploaded to the store
        let key = object_store::path::Path::from(format!("{hash}.webp"));
        let stored = store.get(&key).await.unwrap();
        assert_eq!(stored.bytes().await.unwrap().as_ref(), b"hello world");

        // cdn_objects row was inserted
        let cdn_url = lookup_cdn_object(&pool, &hash).await.unwrap();
        assert_eq!(cdn_url.as_deref(), Some(result.as_str()));
    }

    #[tokio::test]
    async fn store_bytes_on_hit_returns_existing_url_without_uploading() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let bytes = b"hello world".to_vec();
        let hash = blake3_hash_bytes(&bytes);
        let existing_url = "https://cdn.example.com/pre-existing.webp";

        // Pre-seed the cdn_objects table (simulates a prior upload)
        insert_cdn_object(&pool, &hash, existing_url).await.unwrap();

        // Call store_bytes — should hit the cache
        let result = svc.store_bytes(bytes, "webp").await.unwrap();

        // Returns the pre-existing URL, not a newly generated one
        assert_eq!(result, existing_url);

        // B2 was NOT called — store is empty
        let key = object_store::path::Path::from(format!("{hash}.webp"));
        assert!(
            store.get(&key).await.is_err(),
            "B2 should not have been called on a dedup hit"
        );
    }

    #[tokio::test]
    async fn garbage_collect_removes_unreferenced_object_and_mapping() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let bytes = b"orphan me".to_vec();
        let hash = blake3_hash_bytes(&bytes);
        let cdn_url = svc.store_bytes(bytes, "webp").await.unwrap();

        assert_eq!(svc.garbage_collect_orphaned_objects().await.unwrap(), 1);
        assert!(
            store
                .get(&object_store::path::Path::from(format!("{hash}.webp")))
                .await
                .is_err()
        );
        assert!(lookup_cdn_object(&pool, &hash).await.unwrap().is_none());
        assert!(!cdn_url.is_empty());
    }

    #[tokio::test]
    async fn garbage_collect_keeps_shared_object_and_mapping() {
        let pool = setup_test_db().await;
        let store = Arc::new(InMemory::new());
        let svc =
            StorageService::new_with_store(store.clone(), "https://cdn.example.com", pool.clone());

        let bytes = b"keep me".to_vec();
        let hash = blake3_hash_bytes(&bytes);
        let cdn_url = svc.store_bytes(bytes, "webp").await.unwrap();
        for id in ["first", "second"] {
            sqlx::query("INSERT INTO images (id, cdn_url) VALUES (?, ?)")
                .bind(id)
                .bind(&cdn_url)
                .execute(&pool)
                .await
                .unwrap();
        }

        assert_eq!(svc.garbage_collect_orphaned_objects().await.unwrap(), 0);
        assert!(
            store
                .get(&object_store::path::Path::from(format!("{hash}.webp")))
                .await
                .is_ok()
        );
        assert_eq!(
            lookup_cdn_object(&pool, &hash).await.unwrap().as_deref(),
            Some(cdn_url.as_str())
        );
    }
}
