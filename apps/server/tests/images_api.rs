use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use http_body_util::BodyExt;
use memebucket_server::{
    app_state::AppState,
    auth::sessions::AuthenticatedUser,
    config::RootAdminConfig,
    repositories::{
        BucketRepo, ImageRepo, SendHistoryRepo, UserRepo, buckets::BucketRepository,
        images::ImageRepository, users::UserRepository,
    },
    router::build_router_for_tests,
    services::storage::StorageService,
};
use object_store::memory::InMemory;
use sqlx::SqlitePool;
use std::ffi::OsString;
use std::io::Cursor;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower::ServiceExt;

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

static LOCAL_IP_TEST_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvVarGuard {
    name: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::set_var(name, value);
        }
        Self { name, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.name, value);
            },
            None => unsafe {
                std::env::remove_var(self.name);
            },
        }
    }
}

fn sample_png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 255, 0, 255]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

async fn multipart_upload_body(parts: &[(&str, &str, &str, &[u8])]) -> (String, Vec<u8>) {
    let mut form = reqwest::multipart::Form::new();
    for (field_name, file_name, content_type, bytes) in parts {
        let part = reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name((*file_name).to_string())
            .mime_str(content_type)
            .unwrap();
        form = form.part((*field_name).to_string(), part);
    }
    let mut request = reqwest::Client::new()
        .post("http://example.test/upload")
        .multipart(form)
        .build()
        .unwrap();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = request
        .body_mut()
        .as_mut()
        .expect("multipart body")
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (content_type, body)
}

#[tokio::test]
async fn test_bulk_delete_and_move_images() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());

    let user = users
        .upsert_by_provider("discord", "owner", None, None)
        .await
        .unwrap();

    let bucket_a = buckets.create(user.id, "Bucket A").await.unwrap();
    let bucket_b = buckets.create(user.id, "Bucket B").await.unwrap();

    // Create 3 images in bucket A
    let img1 = images_repo
        .create(user.id, bucket_a.id, "https://example.com/1.png")
        .await
        .unwrap();
    let img2 = images_repo
        .create(user.id, bucket_a.id, "https://example.com/2.png")
        .await
        .unwrap();
    let img3 = images_repo
        .create(user.id, bucket_a.id, "https://example.com/3.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool);
    let app = build_router_for_tests(state);

    // 1. Test Bulk Move: move img1 and img2 to bucket B
    let payload_move = serde_json::json!({
        "imageIds": [img1.id.to_string(), img2.id.to_string()],
        "newBucketId": bucket_b.id.to_string()
    });
    let mut move_request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/bulk/move", bucket_a.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload_move).unwrap()))
        .unwrap();
    move_request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let move_response = app.clone().oneshot(move_request).await.unwrap();
    assert_eq!(move_response.status(), StatusCode::OK);

    let move_body = move_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let move_json: serde_json::Value = serde_json::from_slice(&move_body).unwrap();
    assert_eq!(move_json["moved"], 2);

    // Verify images in bucket A and bucket B
    let images_a = images_repo
        .list_for_bucket(user.id, bucket_a.id)
        .await
        .unwrap();
    let images_b = images_repo
        .list_for_bucket(user.id, bucket_b.id)
        .await
        .unwrap();
    assert_eq!(images_a.len(), 1); // Only img3 remains
    assert_eq!(images_a[0].id, img3.id);
    assert_eq!(images_b.len(), 2); // img1 and img2 moved here

    // 2. Test Bulk Delete: delete img1 and img2 from bucket B
    let payload_delete = serde_json::json!({
        "imageIds": [img1.id.to_string(), img2.id.to_string()]
    });
    let mut delete_request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/buckets/{}/images/bulk", bucket_b.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload_delete).unwrap()))
        .unwrap();
    delete_request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let delete_response = app.clone().oneshot(delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);

    let delete_body = delete_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let delete_json: serde_json::Value = serde_json::from_slice(&delete_body).unwrap();
    assert_eq!(delete_json["deleted"], 2);

    // Verify bucket B is now empty
    let images_b_after = images_repo
        .list_for_bucket(user.id, bucket_b.id)
        .await
        .unwrap();
    assert_eq!(images_b_after.len(), 0);
}

#[tokio::test]
async fn update_image_with_url_resolves_and_replaces_content() {
    let _local_ip_guard = LOCAL_IP_TEST_LOCK.lock().await;
    let _allow_local_ips = EnvVarGuard::set("MEMEBUCKET_ALLOW_LOCAL_IPS_IN_TESTS", "1");

    async fn new_image() -> Response {
        ([(header::CONTENT_TYPE, "image/gif")], "new-gif-bytes").into_response()
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app_server = Router::new().route("/new.gif", get(new_image));
    tokio::spawn(async move {
        axum::serve(listener, app_server).await.unwrap();
    });
    let new_url = format!("http://{address}/new.gif");

    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let user = users
        .upsert_by_provider("discord", "url-editor", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(user.id, bucket.id, "https://example.com/old.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    let mut request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/buckets/{}/images/{}", bucket.id, image.id))
        .header("content-type", "application/json")
        .body(Body::from(format!(r#"{{"url":"{new_url}"}}"#)))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let updated = images_repo
        .get_for_owner(user.id, bucket.id, image.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.url, new_url);
    assert_eq!(updated.cdn_url.as_deref(), Some(new_url.as_str()));
    assert_eq!(updated.cdn_status.as_deref(), Some("migrated"));
}

#[tokio::test]
async fn update_image_with_invalid_url_returns_bad_request_and_leaves_row_unchanged() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let user = users
        .upsert_by_provider("discord", "url-editor-invalid", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(user.id, bucket.id, "https://example.com/old.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    let mut request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/buckets/{}/images/{}", bucket.id, image.id))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"url":"not-a-url"}"#))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let unchanged = images_repo
        .get_for_owner(user.id, bucket.id, image.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.url, "https://example.com/old.png");
}

#[tokio::test]
async fn update_image_without_url_leaves_existing_url_untouched() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let user = users
        .upsert_by_provider("discord", "url-editor-notouch", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(user.id, bucket.id, "https://example.com/old.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    let mut request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/buckets/{}/images/{}", bucket.id, image.id))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title":"New Title"}"#))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let unchanged = images_repo
        .get_for_owner(user.id, bucket.id, image.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.url, "https://example.com/old.png");
    assert_eq!(unchanged.title.as_deref(), Some("New Title"));
}

#[tokio::test]
async fn update_image_with_url_resolving_to_video_routes_through_video_path() {
    let _local_ip_guard = LOCAL_IP_TEST_LOCK.lock().await;
    let _allow_local_ips = EnvVarGuard::set("MEMEBUCKET_ALLOW_LOCAL_IPS_IN_TESTS", "1");

    // Serve from a path ending in `.mp4` with a video content-type: `is_video` in
    // resolve_and_upload_url checks the resolved URL's suffix, and
    // `resolve_image_url` only accepts the URL directly (without HTML-scraping)
    // when the live content-type starts with `video/`.
    async fn video() -> Response {
        ([(header::CONTENT_TYPE, "video/mp4")], "fake-mp4-bytes").into_response()
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app_server = Router::new().route("/clip.mp4", get(video));
    tokio::spawn(async move {
        axum::serve(listener, app_server).await.unwrap();
    });
    let video_url = format!("http://{address}/clip.mp4");

    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let user = users
        .upsert_by_provider("discord", "url-editor-video", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(user.id, bucket.id, "https://example.com/old.png")
        .await
        .unwrap();

    // No storage configured in AppState::for_tests, so the video force-upload
    // branch is skipped (state.storage() is None) — this test proves the URL
    // still resolves to a `.mp4` and is accepted, without requiring real B2/ffmpeg.
    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    let mut request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/buckets/{}/images/{}", bucket.id, image.id))
        .header("content-type", "application/json")
        .body(Body::from(format!(r#"{{"url":"{video_url}"}}"#)))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let updated = images_repo
        .get_for_owner(user.id, bucket.id, image.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.url, video_url);
}

#[tokio::test]
async fn record_image_send_inserts_row_and_updates_send_count() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());

    let user = users
        .upsert_by_provider("discord", "owner", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(user.id, bucket.id, "https://example.com/1.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    let mut request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/buckets/{}/images/{}/send",
            bucket.id, image.id
        ))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["recorded"], true);

    let send_history =
        memebucket_server::repositories::send_history::SendHistoryRepository::new(pool);
    let counts = send_history
        .count_for_images(user.id, &[image.id])
        .await
        .unwrap();
    assert_eq!(counts.get(&image.id).copied(), Some(1));
}

#[tokio::test]
async fn record_image_send_for_inaccessible_image_returns_not_found() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());

    let owner = users
        .upsert_by_provider("discord", "owner", None, None)
        .await
        .unwrap();
    let stranger = users
        .upsert_by_provider("discord", "stranger", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(owner.id, bucket.id, "https://example.com/1.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool);
    let app = build_router_for_tests(state);

    let mut request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/buckets/{}/images/{}/send",
            bucket.id, image.id
        ))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: stranger.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn record_image_send_debounces_rapid_duplicate_selection() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());

    let user = users
        .upsert_by_provider("discord", "owner", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();
    let image = images_repo
        .create(user.id, bucket.id, "https://example.com/1.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    for expected_recorded in [true, false] {
        let mut request = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/buckets/{}/images/{}/send",
                bucket.id, image.id
            ))
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(AuthenticatedUser {
            user_id: user.id,
            role: "user".to_string(),
        });

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["recorded"], expected_recorded);
    }

    let send_history =
        memebucket_server::repositories::send_history::SendHistoryRepository::new(pool);
    let counts = send_history
        .count_for_images(user.id, &[image.id])
        .await
        .unwrap();
    assert_eq!(counts.get(&image.id).copied(), Some(1));
}

#[tokio::test]
async fn record_image_send_rate_limits_after_thirty_in_one_minute() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());

    let user = users
        .upsert_by_provider("discord", "owner", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(user.id, "Bucket").await.unwrap();

    let mut image_ids = Vec::new();
    for i in 0..31 {
        let image = images_repo
            .create(user.id, bucket.id, &format!("https://example.com/{i}.png"))
            .await
            .unwrap();
        image_ids.push(image.id);
    }

    let state = AppState::for_tests(pool);
    let app = build_router_for_tests(state);

    for (index, image_id) in image_ids.iter().enumerate() {
        let mut request = Request::builder()
            .method("POST")
            .uri(format!(
                "/api/buckets/{}/images/{}/send",
                bucket.id, image_id
            ))
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(AuthenticatedUser {
            user_id: user.id,
            role: "user".to_string(),
        });

        let response = app.clone().oneshot(request).await.unwrap();
        if index < 30 {
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "request {index} should succeed"
            );
        } else {
            assert_eq!(
                response.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "request {index} should be rate limited"
            );
        }
    }
}

#[tokio::test]
async fn move_image_rejects_snake_case_body_and_accepts_bucket_id() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());

    let user = users
        .upsert_by_provider("discord", "owner", None, None)
        .await
        .unwrap();
    let bucket_a = buckets.create(user.id, "Bucket A").await.unwrap();
    let bucket_b = buckets.create(user.id, "Bucket B").await.unwrap();
    let image = images_repo
        .create(user.id, bucket_a.id, "https://example.com/1.png")
        .await
        .unwrap();

    let state = AppState::for_tests(pool.clone());
    let app = build_router_for_tests(state);

    // The frontend used to send `new_bucket_id` (snake_case) — the server's
    // `MoveImageRequest` only ever accepted `bucketId`, so this shape was
    // silently 422ing in production. Pin that this is rejected.
    let bad_payload = serde_json::json!({ "new_bucket_id": bucket_b.id.to_string() });
    let mut bad_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/buckets/{}/images/{}/move",
            bucket_a.id, image.id
        ))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&bad_payload).unwrap()))
        .unwrap();
    bad_request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let bad_response = app.clone().oneshot(bad_request).await.unwrap();
    assert_eq!(bad_response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // The correct shape (`bucketId`) succeeds and actually performs the move.
    let good_payload = serde_json::json!({ "bucketId": bucket_b.id.to_string() });
    let mut good_request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/buckets/{}/images/{}/move",
            bucket_a.id, image.id
        ))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&good_payload).unwrap()))
        .unwrap();
    good_request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: "user".to_string(),
    });

    let good_response = app.clone().oneshot(good_request).await.unwrap();
    assert_eq!(good_response.status(), StatusCode::OK);

    let images_b = images_repo
        .list_for_bucket(user.id, bucket_b.id)
        .await
        .unwrap();
    assert_eq!(images_b.len(), 1);
    assert_eq!(images_b[0].id, image.id);
}

#[tokio::test]
async fn upload_image_permitted_owner_creates_image_row() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(owner.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let (content_type, body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &sample_png_bytes())]).await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        json["url"].as_str().unwrap().ends_with(".webp"),
        "expected WebP URL, got {json}"
    );

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].title.as_deref(), Some(""));
    assert!(images[0].tags.is_empty());
}

#[tokio::test]
async fn upload_image_duplicate_upload_creates_second_row_with_same_url() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-duplicate", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(owner.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let source_bytes = sample_png_bytes();
    let (content_type, first_body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &source_bytes)]).await;
    let mut first_request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(first_body))
        .unwrap();
    first_request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let first_response = app.clone().oneshot(first_request).await.unwrap();
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body = first_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let first_json: serde_json::Value = serde_json::from_slice(&first_body).unwrap();

    let (content_type, second_body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &source_bytes)]).await;
    let mut second_request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(second_body))
        .unwrap();
    second_request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let second_response = app.clone().oneshot(second_request).await.unwrap();
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = second_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let second_json: serde_json::Value = serde_json::from_slice(&second_body).unwrap();

    assert_eq!(first_json["url"], second_json["url"]);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].url, images[1].url);
    assert_ne!(images[0].id, images[1].id);
}

#[tokio::test]
async fn upload_image_configured_root_admin_owner_succeeds_without_explicit_permission_row() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-root-owner", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state = AppState::for_tests(pool.clone())
        .with_root_admin_config(RootAdminConfig::parse("discord:upload-root-owner").unwrap())
        .with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    let app = build_router_for_tests(state);

    let (content_type, body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &sample_png_bytes())]).await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert_eq!(images.len(), 1);
}

#[tokio::test]
async fn upload_image_without_permission_returns_forbidden() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-no-perm", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    let app = build_router_for_tests(state);

    let (content_type, body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &sample_png_bytes())]).await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert!(images.is_empty());
}

#[tokio::test]
async fn upload_image_non_owner_returns_forbidden() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-only", None, None)
        .await
        .unwrap();
    let stranger = users
        .upsert_by_provider("discord", "upload-stranger", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(stranger.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let (content_type, body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &sample_png_bytes())]).await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: stranger.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert!(images.is_empty());
}

#[tokio::test]
async fn upload_image_rejects_payload_over_twenty_mib() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-large", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(owner.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let oversized = vec![0_u8; 20 * 1024 * 1024 + 1];
    let (content_type, body) =
        multipart_upload_body(&[("file", "upload.png", "image/png", &oversized)]).await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert!(images.is_empty());
}

#[tokio::test]
async fn upload_image_rejects_malformed_image_bytes_without_creating_row() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-bad-bytes", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(owner.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let (content_type, body) = multipart_upload_body(&[(
        "file",
        "upload.png",
        "image/png",
        b"definitely-not-a-real-image",
    )])
    .await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert!(images.is_empty());
}

#[tokio::test]
async fn upload_image_rejects_wrong_multipart_field_name() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-wrong-field", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(owner.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let (content_type, body) =
        multipart_upload_body(&[("image", "upload.png", "image/png", &sample_png_bytes())]).await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert!(images.is_empty());
}

#[tokio::test]
async fn upload_image_rejects_second_multipart_field() {
    let pool = test_pool().await;
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images_repo = ImageRepository::new(pool.clone());
    let owner = users
        .upsert_by_provider("discord", "upload-owner-extra-field", None, None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Bucket").await.unwrap();

    let state =
        AppState::for_tests(pool.clone()).with_storage(Some(StorageService::new_with_store(
            Arc::new(InMemory::new()),
            "https://cdn.example.com",
            pool.clone(),
        )));
    state
        .admin_repo
        .set_permission(owner.id, "upload_local_images", true)
        .await
        .unwrap();
    let app = build_router_for_tests(state);

    let extra_bytes = b"not-used";
    let (content_type, body) = multipart_upload_body(&[
        ("file", "upload.png", "image/png", &sample_png_bytes()),
        ("note", "note.txt", "text/plain", extra_bytes),
    ])
    .await;
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/buckets/{}/images/upload", bucket.id))
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: owner.id,
        role: "user".to_string(),
    });

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let images = images_repo
        .list_for_bucket(owner.id, bucket.id)
        .await
        .unwrap();
    assert!(images.is_empty());
}
