use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use memebucket_server::repositories::{
    BucketRepo, ImageRepo, UserRepo, admin_stats::AdminStatsRepository, buckets::BucketRepository,
    images::ImageRepository, users::UserRepository,
};
use memebucket_server::{
    app_state::AppState,
    auth::sessions::AuthenticatedUser,
    config::RootAdminConfig,
    repositories::users::StoredUser,
    router::build_router_for_tests,
    services::{admin_stats::AdminStatsService, storage::StorageService},
};
use object_store::{ObjectStoreExt, memory::InMemory, path::Path as ObjectPath};
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const ROOT_PROVIDER_ID: &str = "admin-stats-root-provider-id";

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

struct StatsApiFixture {
    app: Router,
    pool: SqlitePool,
    root: StoredUser,
    normal_admin: StoredUser,
}

async fn stats_api_fixture() -> StatsApiFixture {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;

    let users = UserRepository::new(pool.clone());
    let root = users
        .upsert_by_provider("discord", ROOT_PROVIDER_ID, Some("Root Admin"), None)
        .await
        .unwrap();
    let normal_admin = users
        .upsert_by_provider(
            "discord",
            "admin-stats-normal-admin",
            Some("Normal Admin"),
            None,
        )
        .await
        .unwrap();

    sqlx::query("UPDATE users SET role = 'admin' WHERE id = ?")
        .bind(normal_admin.id.to_string())
        .execute(&pool)
        .await
        .unwrap();

    let normal_admin = users.get_by_id(normal_admin.id).await.unwrap().unwrap();
    let state = AppState::for_tests(pool.clone()).with_root_admin_config(
        RootAdminConfig::parse(&format!("discord:{ROOT_PROVIDER_ID}")).unwrap(),
    );
    let service = state.admin_stats_service();
    service
        .refresh_snapshot(NaiveDate::from_ymd_opt(2026, 7, 27).unwrap())
        .await
        .unwrap();
    service
        .refresh_snapshot(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap())
        .await
        .unwrap();

    StatsApiFixture {
        app: build_router_for_tests(state),
        pool,
        root,
        normal_admin,
    }
}

async fn get_stats(app: &Router, user: &StoredUser) -> Response {
    let mut request = Request::builder()
        .uri("/api/admin/stats")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: user.id,
        role: user.role.clone(),
    });
    app.clone().oneshot(request).await.unwrap()
}

async fn read_json(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn snapshot_dates(pool: &SqlitePool) -> Vec<NaiveDate> {
    AdminStatsRepository::new(pool.clone())
        .list_snapshots()
        .await
        .unwrap()
        .into_iter()
        .map(|snapshot| snapshot.snapshot_date)
        .collect()
}

#[tokio::test]
async fn refresh_current_snapshot_counts_service_wide_records() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;

    let repo = AdminStatsRepository::new(pool.clone());
    let snapshot = repo
        .refresh_current_snapshot(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap())
        .await
        .unwrap();

    assert_eq!(snapshot.user_count, 2);
    assert_eq!(snapshot.bucket_count, 3);
    assert_eq!(snapshot.image_link_count, 4);
    assert_eq!(snapshot.send_count, 5);
}

#[tokio::test]
async fn refreshing_same_day_updates_one_snapshot_row() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;
    let repo = AdminStatsRepository::new(pool.clone());
    let date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();

    repo.refresh_current_snapshot(date).await.unwrap();
    insert_image_for_fixture_user(&pool).await;
    let refreshed = repo.refresh_current_snapshot(date).await.unwrap();
    let rows = repo.list_snapshots().await.unwrap();

    assert_eq!(
        rows.iter().filter(|row| row.snapshot_date == date).count(),
        1
    );
    assert_eq!(refreshed.image_link_count, 5);
}

#[tokio::test]
async fn historical_backfill_leaves_unavailable_file_metrics_null() {
    let pool = test_pool().await;
    seed_dated_stats_fixture(&pool).await;
    let repo = AdminStatsRepository::new(pool.clone());

    repo.backfill_historical_snapshots().await.unwrap();
    let rows = repo.list_snapshots().await.unwrap();
    let july_26 = snapshot_on(&rows, NaiveDate::from_ymd_opt(2026, 7, 26).unwrap());
    let july_27 = snapshot_on(&rows, NaiveDate::from_ymd_opt(2026, 7, 27).unwrap());
    let july_28 = snapshot_on(&rows, NaiveDate::from_ymd_opt(2026, 7, 28).unwrap());

    assert_eq!(july_26.user_count, 1);
    assert_eq!(july_26.bucket_count, 0);
    assert_eq!(july_26.image_link_count, 0);
    assert_eq!(july_26.send_count, 0);
    assert!(july_26.unique_file_count.is_none());
    assert!(july_26.b2_object_count.is_none());
    assert!(july_26.b2_bytes.is_none());

    assert_eq!(july_27.user_count, 1);
    assert_eq!(july_27.bucket_count, 1);
    assert_eq!(july_27.image_link_count, 0);
    assert_eq!(july_27.send_count, 0);
    assert!(july_27.unique_file_count.is_none());
    assert!(july_27.b2_object_count.is_none());
    assert!(july_27.b2_bytes.is_none());

    assert_eq!(july_28.user_count, 1);
    assert_eq!(july_28.bucket_count, 1);
    assert_eq!(july_28.image_link_count, 1);
    assert_eq!(july_28.send_count, 1);
    assert!(july_28.unique_file_count.is_none());
    assert!(july_28.b2_object_count.is_none());
    assert!(july_28.b2_bytes.is_none());
}

#[tokio::test]
async fn refreshing_same_day_updates_unique_file_count_to_zero_and_preserves_b2_metrics() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;
    let repo = AdminStatsRepository::new(pool.clone());
    let date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();

    seed_snapshot_row(
        &pool,
        date,
        SnapshotSeed {
            user_count: 1,
            bucket_count: 1,
            image_link_count: 1,
            unique_file_count: Some(9),
            send_count: 1,
            b2_object_count: Some(11),
            b2_bytes: Some(222),
        },
    )
    .await;
    insert_cdn_object(&pool, "fixture-hash-a", "https://cdn.example.com/a.gif").await;
    clear_cdn_objects(&pool).await;
    insert_image_for_fixture_user(&pool).await;

    let refreshed = repo.refresh_current_snapshot(date).await.unwrap();

    assert_eq!(refreshed.user_count, 2);
    assert_eq!(refreshed.bucket_count, 3);
    assert_eq!(refreshed.image_link_count, 5);
    assert_eq!(refreshed.send_count, 5);
    assert_eq!(refreshed.unique_file_count, Some(0));
    assert_eq!(refreshed.b2_object_count, Some(11));
    assert_eq!(refreshed.b2_bytes, Some(222));
}

#[tokio::test]
async fn historical_backfill_uses_explicit_utc_dates_at_midnight_boundaries() {
    let pool = test_pool().await;
    seed_explicit_utc_boundary_fixture(&pool).await;
    let repo = AdminStatsRepository::new(pool.clone());

    repo.backfill_historical_snapshots().await.unwrap();
    let rows = repo.list_snapshots().await.unwrap();
    let july_27 = snapshot_on(&rows, NaiveDate::from_ymd_opt(2026, 7, 27).unwrap());
    let july_28 = snapshot_on(&rows, NaiveDate::from_ymd_opt(2026, 7, 28).unwrap());

    assert_eq!(july_27.user_count, 1);
    assert_eq!(july_27.bucket_count, 0);
    assert_eq!(july_27.image_link_count, 0);
    assert_eq!(july_27.send_count, 0);

    assert_eq!(july_28.user_count, 1);
    assert_eq!(july_28.bucket_count, 1);
    assert_eq!(july_28.image_link_count, 1);
    assert_eq!(july_28.send_count, 1);
}

#[tokio::test]
async fn storage_stats_sums_object_metadata_without_downloading_objects() {
    let storage = test_storage_with_objects(
        test_pool().await,
        &[("a.webp", 10), ("nested/deep.webp", 7), ("b.webp", 25)],
    )
    .await;

    let stats = storage.list_stats().await.unwrap();

    assert_eq!(stats.object_count, 3);
    assert_eq!(stats.bytes, 42);
}

#[tokio::test]
async fn refresh_snapshot_without_storage_keeps_database_metrics_available() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;
    insert_cdn_object(&pool, "fixture-hash-a", "https://cdn.example.com/a.webp").await;

    let repo = AdminStatsRepository::new(pool.clone());
    let service = AdminStatsService::new(repo, None);

    let refreshed = service
        .refresh_snapshot(NaiveDate::from_ymd_opt(2026, 7, 28).unwrap())
        .await
        .unwrap();

    assert_eq!(refreshed.snapshot.user_count, 2);
    assert_eq!(refreshed.snapshot.bucket_count, 3);
    assert_eq!(refreshed.snapshot.image_link_count, 4);
    assert_eq!(refreshed.snapshot.unique_file_count, Some(1));
    assert_eq!(refreshed.snapshot.send_count, 5);
    assert!(refreshed.snapshot.b2_object_count.is_none());
    assert!(refreshed.snapshot.b2_bytes.is_none());
    assert!(!refreshed.storage_available);
}

#[tokio::test]
async fn refresh_snapshot_with_storage_persists_provider_metrics_through_repository() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;
    insert_cdn_object(&pool, "fixture-hash-a", "https://cdn.example.com/a.webp").await;

    let repo = AdminStatsRepository::new(pool.clone());
    let storage =
        test_storage_with_objects(test_pool().await, &[("a.webp", 10), ("b.webp", 25)]).await;
    let service = AdminStatsService::new(repo, Some(storage));
    let date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();

    let refreshed = service.refresh_snapshot(date).await.unwrap();
    let history = service.load_history().await.unwrap();
    let saved = snapshot_on(&history, date);

    assert_eq!(refreshed.snapshot.unique_file_count, Some(1));
    assert_eq!(refreshed.snapshot.b2_object_count, Some(2));
    assert_eq!(refreshed.snapshot.b2_bytes, Some(35));
    assert_eq!(saved.b2_object_count, Some(2));
    assert_eq!(saved.b2_bytes, Some(35));
    assert!(refreshed.storage_available);
}

#[tokio::test]
async fn b2_listing_failure_preserves_last_known_snapshot() {
    let pool = test_pool().await;
    seed_stats_fixture(&pool).await;
    insert_cdn_object(&pool, "fixture-hash-a", "https://cdn.example.com/a.webp").await;
    insert_cdn_object(&pool, "fixture-hash-b", "https://cdn.example.com/b.webp").await;
    insert_image_for_fixture_user(&pool).await;
    let date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();

    seed_snapshot_row(
        &pool,
        date,
        SnapshotSeed {
            user_count: 1,
            bucket_count: 1,
            image_link_count: 1,
            unique_file_count: Some(9),
            send_count: 1,
            b2_object_count: Some(11),
            b2_bytes: Some(222),
        },
    )
    .await;

    let repo = AdminStatsRepository::new(pool.clone());
    let service = AdminStatsService::new(repo, Some(failing_list_storage(pool.clone()).await));

    let refreshed = service.refresh_snapshot(date).await.unwrap();
    let history = service.load_history().await.unwrap();
    let saved = snapshot_on(&history, date);

    assert_eq!(refreshed.snapshot.user_count, 2);
    assert_eq!(refreshed.snapshot.bucket_count, 3);
    assert_eq!(refreshed.snapshot.image_link_count, 5);
    assert_eq!(refreshed.snapshot.unique_file_count, Some(2));
    assert_eq!(refreshed.snapshot.send_count, 5);
    assert_eq!(refreshed.snapshot.b2_object_count, Some(11));
    assert_eq!(refreshed.snapshot.b2_bytes, Some(222));
    assert!(!refreshed.storage_available);
    assert_eq!(saved.b2_object_count, Some(11));
    assert_eq!(saved.b2_bytes, Some(222));
}

#[tokio::test]
async fn stats_requires_view_permission_for_normal_admin() {
    let fixture = stats_api_fixture().await;

    let response = get_stats(&fixture.app, &fixture.normal_admin).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn stats_allows_root_admin_and_returns_aggregate_history() {
    let fixture = stats_api_fixture().await;
    let before_dates = snapshot_dates(&fixture.pool).await;

    let response = get_stats(&fixture.app, &fixture.root).await;
    let after_dates = snapshot_dates(&fixture.pool).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let history = body["history"].as_array().unwrap();

    assert!(body["current"]["user_count"].is_number());
    assert_eq!(body["current"]["snapshot_date"], "2026-07-28");
    assert!(body["history"].is_array());
    assert!(body["storage"]["configured"].is_boolean());
    assert!(body["storage"]["available"].is_boolean());
    assert!(body.get("username").is_none());
    assert!(body.get("provider_user_id").is_none());
    assert_eq!(before_dates.len(), 2);
    assert_eq!(
        before_dates,
        vec![
            NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
        ]
    );
    assert_eq!(after_dates, before_dates);
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["snapshot_date"], "2026-07-27");
    assert_eq!(history[1]["snapshot_date"], "2026-07-28");
    assert!(body["storage"]["first_complete_history_date"].is_null());
}

async fn seed_stats_fixture(pool: &SqlitePool) {
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images = ImageRepository::new(pool.clone());

    let owner_one = users
        .upsert_by_provider("discord", "admin-stats-owner-one", Some("Owner One"), None)
        .await
        .unwrap();
    let owner_two = users
        .upsert_by_provider("discord", "admin-stats-owner-two", Some("Owner Two"), None)
        .await
        .unwrap();

    let bucket_one = buckets
        .create(owner_one.id, "Owner One Primary")
        .await
        .unwrap();
    let bucket_two = buckets
        .create(owner_one.id, "Owner One Secondary")
        .await
        .unwrap();
    let bucket_three = buckets
        .create(owner_two.id, "Owner Two Primary")
        .await
        .unwrap();

    let image_one = images
        .create(owner_one.id, bucket_one.id, "https://example.com/1.gif")
        .await
        .unwrap();
    let image_two = images
        .create(owner_one.id, bucket_one.id, "https://example.com/2.gif")
        .await
        .unwrap();
    let image_three = images
        .create(owner_one.id, bucket_two.id, "https://example.com/3.gif")
        .await
        .unwrap();
    let image_four = images
        .create(owner_two.id, bucket_three.id, "https://example.com/4.gif")
        .await
        .unwrap();

    insert_send(
        pool,
        owner_one.id,
        bucket_one.id,
        image_one.id,
        &bucket_one.name,
        &image_one.url,
        "2026-07-28 01:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_one.id,
        bucket_one.id,
        image_two.id,
        &bucket_one.name,
        &image_two.url,
        "2026-07-28 02:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_one.id,
        bucket_two.id,
        image_three.id,
        &bucket_two.name,
        &image_three.url,
        "2026-07-28 03:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_two.id,
        bucket_three.id,
        image_four.id,
        &bucket_three.name,
        &image_four.url,
        "2026-07-28 04:00:00",
    )
    .await;
    insert_send(
        pool,
        owner_two.id,
        bucket_three.id,
        image_four.id,
        &bucket_three.name,
        &image_four.url,
        "2026-07-28 05:00:00",
    )
    .await;
}

async fn insert_image_for_fixture_user(pool: &SqlitePool) {
    let (owner_id, bucket_id): (String, String) = sqlx::query_as(
        "SELECT owner_user_id, id
         FROM buckets
         WHERE name = 'Owner One Primary'",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let images = ImageRepository::new(pool.clone());
    images
        .create(
            Uuid::parse_str(&owner_id).unwrap(),
            Uuid::parse_str(&bucket_id).unwrap(),
            "https://example.com/5.gif",
        )
        .await
        .unwrap();
}

async fn seed_dated_stats_fixture(pool: &SqlitePool) {
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images = ImageRepository::new(pool.clone());

    let owner = users
        .upsert_by_provider("discord", "dated-owner", Some("Dated Owner"), None)
        .await
        .unwrap();
    let bucket = buckets.create(owner.id, "Historical Bucket").await.unwrap();
    let image = images
        .create(owner.id, bucket.id, "https://example.com/historical.gif")
        .await
        .unwrap();

    set_timestamp(
        pool,
        "users",
        owner.id,
        "2026-07-26 12:00:00",
        Some("updated_at"),
    )
    .await;
    set_timestamp(
        pool,
        "buckets",
        bucket.id,
        "2026-07-27 09:00:00",
        Some("updated_at"),
    )
    .await;
    set_timestamp(pool, "images", image.id, "2026-07-28 10:00:00", None).await;
    insert_send(
        pool,
        owner.id,
        bucket.id,
        image.id,
        &bucket.name,
        &image.url,
        "2026-07-28 11:00:00",
    )
    .await;
}

async fn seed_explicit_utc_boundary_fixture(pool: &SqlitePool) {
    let users = UserRepository::new(pool.clone());
    let buckets = BucketRepository::new(pool.clone());
    let images = ImageRepository::new(pool.clone());

    let owner = users
        .upsert_by_provider("discord", "utc-boundary-owner", Some("UTC Boundary"), None)
        .await
        .unwrap();
    let bucket = buckets
        .create(owner.id, "UTC Boundary Bucket")
        .await
        .unwrap();
    let image = images
        .create(owner.id, bucket.id, "https://example.com/utc-boundary.gif")
        .await
        .unwrap();

    set_timestamp(
        pool,
        "users",
        owner.id,
        "2026-07-27T23:59:59Z",
        Some("updated_at"),
    )
    .await;
    set_timestamp(
        pool,
        "buckets",
        bucket.id,
        "2026-07-28T00:00:00Z",
        Some("updated_at"),
    )
    .await;
    set_timestamp(pool, "images", image.id, "2026-07-28T00:00:01Z", None).await;
    insert_send(
        pool,
        owner.id,
        bucket.id,
        image.id,
        &bucket.name,
        &image.url,
        "2026-07-28T00:00:02Z",
    )
    .await;
}

async fn test_storage_with_objects(pool: SqlitePool, objects: &[(&str, usize)]) -> StorageService {
    let store = Arc::new(InMemory::new());

    for (key, size) in objects {
        store
            .put(&ObjectPath::from(*key), vec![b'x'; *size].into())
            .await
            .unwrap();
    }

    StorageService::new_with_store(store, "https://cdn.example.com", pool)
}

async fn failing_list_storage(pool: SqlitePool) -> StorageService {
    StorageService::new(
        "fixture-bucket",
        "127.0.0.1:1",
        "fixture-key-id",
        "fixture-app-key",
        "https://cdn.example.com",
        pool,
    )
    .unwrap()
}

fn snapshot_on(
    snapshots: &[memebucket_server::repositories::admin_stats::AdminStatsSnapshot],
    date: NaiveDate,
) -> &memebucket_server::repositories::admin_stats::AdminStatsSnapshot {
    snapshots
        .iter()
        .find(|snapshot| snapshot.snapshot_date == date)
        .unwrap()
}

struct SnapshotSeed {
    user_count: i64,
    bucket_count: i64,
    image_link_count: i64,
    unique_file_count: Option<i64>,
    send_count: i64,
    b2_object_count: Option<i64>,
    b2_bytes: Option<i64>,
}

async fn seed_snapshot_row(pool: &SqlitePool, date: NaiveDate, seed: SnapshotSeed) {
    sqlx::query(
        "INSERT INTO admin_stats_snapshots
            (snapshot_date, user_count, bucket_count, image_link_count, unique_file_count, send_count, b2_object_count, b2_bytes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(date.to_string())
    .bind(seed.user_count)
    .bind(seed.bucket_count)
    .bind(seed.image_link_count)
    .bind(seed.unique_file_count)
    .bind(seed.send_count)
    .bind(seed.b2_object_count)
    .bind(seed.b2_bytes)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_cdn_object(pool: &SqlitePool, content_hash: &str, cdn_url: &str) {
    sqlx::query("INSERT INTO cdn_objects (content_hash, cdn_url) VALUES (?, ?)")
        .bind(content_hash)
        .bind(cdn_url)
        .execute(pool)
        .await
        .unwrap();
}

async fn clear_cdn_objects(pool: &SqlitePool) {
    sqlx::query("DELETE FROM cdn_objects")
        .execute(pool)
        .await
        .unwrap();
}

async fn set_timestamp(
    pool: &SqlitePool,
    table: &str,
    id: Uuid,
    timestamp: &str,
    updated_column: Option<&str>,
) {
    match (table, updated_column) {
        ("users", Some("updated_at")) => {
            sqlx::query("UPDATE users SET created_at = ?, updated_at = ? WHERE id = ?")
                .bind(timestamp)
                .bind(timestamp)
                .bind(id.to_string())
                .execute(pool)
                .await
                .unwrap();
        }
        ("buckets", Some("updated_at")) => {
            sqlx::query("UPDATE buckets SET created_at = ?, updated_at = ? WHERE id = ?")
                .bind(timestamp)
                .bind(timestamp)
                .bind(id.to_string())
                .execute(pool)
                .await
                .unwrap();
        }
        ("images", None) => {
            sqlx::query("UPDATE images SET created_at = ? WHERE id = ?")
                .bind(timestamp)
                .bind(id.to_string())
                .execute(pool)
                .await
                .unwrap();
        }
        _ => panic!("unsupported timestamp target: {table} {updated_column:?}"),
    }
}

async fn insert_send(
    pool: &SqlitePool,
    owner_user_id: Uuid,
    bucket_id: Uuid,
    image_id: Uuid,
    bucket_name: &str,
    url: &str,
    sent_at: &str,
) {
    sqlx::query(
        "INSERT INTO send_history
            (id, owner_user_id, bucket_id, image_id, bucket_name, url, response_visibility, sent_at)
         VALUES (?, ?, ?, ?, ?, ?, 'public', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(owner_user_id.to_string())
    .bind(bucket_id.to_string())
    .bind(image_id.to_string())
    .bind(bucket_name)
    .bind(url)
    .bind(sent_at)
    .execute(pool)
    .await
    .unwrap();
}
