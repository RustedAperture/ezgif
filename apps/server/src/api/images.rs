use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    api::ValidatedJson,
    app_state::AppState,
    auth::sessions::AuthenticatedUser,
    error::AppError,
    repositories::{
        SendHistoryRepo,
        images::{
            BulkImageMetadataPatch, ImageSearchFilters, StoredImage, UpdateImageMetadataPatch,
        },
        send_history::SendHistoryRepository,
    },
    services::{images::resolve_image_url, storage::StorageError, storage::StorageService},
};
use validator::Validate;

const PICKER_DEBOUNCE_WINDOW_SECS: i64 = 3;
const PICKER_RATE_LIMIT_WINDOW_SECS: i64 = 60;
const PICKER_RATE_LIMIT_MAX: i64 = 30;
const MAX_LOCAL_UPLOAD_BYTES: usize = 20 * 1024 * 1024;

#[derive(Deserialize, Validate)]
pub struct CreateImageRequest {
    #[validate(url(message = "URL must be a valid HTTP or HTTPS URL"))]
    pub url: String,
    #[validate(custom(function = validate_optional_title))]
    pub title: Option<String>,
    #[validate(custom(function = validate_optional_tags))]
    pub tags: Option<Vec<String>>,
}

fn validate_optional_title(title: &str) -> Result<(), validator::ValidationError> {
    if title.chars().count() > 200 {
        let mut err = validator::ValidationError::new("title_too_long");
        err.message = Some("title must be 200 characters or fewer".into());
        return Err(err);
    }
    Ok(())
}

fn validate_optional_tags(tags: &Vec<String>) -> Result<(), validator::ValidationError> {
    for tag in tags {
        if tag.trim().is_empty() {
            let mut err = validator::ValidationError::new("empty_tag");
            err.message = Some("tags cannot contain empty or blank strings".into());
            return Err(err);
        }
        if tag.chars().count() > 32 {
            let mut err = validator::ValidationError::new("tag_too_long");
            err.message = Some("tags must be 32 characters or fewer".into());
            return Err(err);
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct ImageResponse {
    pub id: String,
    pub url: String,
    pub cdn_status: Option<String>,
    pub title: Option<String>,
    pub favorite: bool,
    #[serde(rename = "randomWeight")]
    pub random_weight: i64,
    pub tags: Vec<String>,
    #[serde(rename = "sendCount")]
    pub send_count: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateImageRequest {
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub title: Option<Option<String>>,
    pub favorite: Option<bool>,
    #[serde(rename = "randomWeight")]
    pub random_weight: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub url: Option<String>,
}

#[derive(Deserialize)]
pub struct MoveImageRequest {
    #[serde(rename = "bucketId")]
    pub new_bucket_id: Uuid,
}

#[derive(Deserialize)]
pub struct BulkUpdateImagesRequest {
    #[serde(rename = "imageIds")]
    pub image_ids: Vec<Uuid>,
    pub favorite: Option<bool>,
    #[serde(rename = "randomWeight")]
    pub random_weight: Option<i64>,
    #[serde(rename = "addTags", default)]
    pub add_tags: Vec<String>,
    #[serde(rename = "removeTags", default)]
    pub remove_tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct BulkDeleteImagesRequest {
    #[serde(rename = "imageIds")]
    pub image_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
pub struct BulkMoveImagesRequest {
    #[serde(rename = "imageIds")]
    pub image_ids: Vec<Uuid>,
    #[serde(rename = "newBucketId")]
    pub new_bucket_id: Uuid,
}

#[derive(Deserialize)]
pub struct SearchImagesQuery {
    pub q: Option<String>,
    #[serde(rename = "bucketId")]
    pub bucket_id: Option<Uuid>,
    pub favorite: Option<bool>,
    #[serde(rename = "randomEnabled")]
    pub random_enabled: Option<bool>,
    pub tags: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct ImageSearchResponse {
    #[serde(rename = "bucketId")]
    pub bucket_id: String,
    #[serde(rename = "bucketName")]
    pub bucket_name: String,
    pub image: ImageResponse,
}

pub async fn list_images(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(bucket_id): Path<Uuid>,
) -> Result<Json<Vec<ImageResponse>>, AppError> {
    let repo = state.image_repo.clone();
    let images = repo.list_for_bucket(user.user_id, bucket_id).await?;
    Ok(Json(
        build_image_responses(state.pool, Some(user.user_id), images).await?,
    ))
}

pub async fn search_images(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(query): Query<SearchImagesQuery>,
) -> Result<Json<Vec<ImageSearchResponse>>, AppError> {
    let repo = state.image_repo.clone();
    let filters = ImageSearchFilters {
        query: query.q,
        bucket_id: query.bucket_id,
        favorite: query.favorite,
        random_enabled: query.random_enabled,
        tags: parse_tag_filter(query.tags),
        limit: i64::from(query.limit.unwrap_or(50).clamp(1, 100)),
    };
    let results = repo.search_for_user(user.user_id, &filters).await?;
    let image_ids = results
        .iter()
        .map(|result| result.image.id)
        .collect::<Vec<_>>();
    let send_counts = state
        .send_history_repo
        .count_for_images(user.user_id, &image_ids)
        .await?;

    Ok(Json(
        results
            .into_iter()
            .map(|result| {
                let send_count = send_counts
                    .get(&result.image.id)
                    .copied()
                    .unwrap_or_default();
                ImageSearchResponse {
                    bucket_id: result.image.bucket_id.to_string(),
                    bucket_name: result.bucket_name,
                    image: image_response_from_stored(result.image, send_count),
                }
            })
            .collect(),
    ))
}

async fn resolve_and_upload_url(
    state: &AppState,
    submitted_url: &str,
) -> Result<(String, Option<String>, Vec<String>), AppError> {
    let resolved = resolve_image_url(submitted_url)
        .await
        .map_err(|err| AppError::BadRequest(err.user_message().to_string()))?;
    let mut resolved_url = resolved.url;
    let notes = resolved.notes;
    let tags = resolved.tags;

    let is_video = crate::services::video_converter::is_video_url(&resolved_url);
    let should_rehost_image = !is_video
        && (StorageService::is_twitter_media(&resolved_url)
            || StorageService::is_bluesky_media(&resolved_url));

    if is_video && let Some(storage) = state.storage() {
        resolved_url =
            crate::services::video_converter::convert_and_upload_video(&resolved_url, storage)
                .await
                .map_err(|err| {
                    AppError::InternalServerError(format!("Failed to convert video: {}", err))
                })?;
    } else if should_rehost_image && let Some(storage) = state.storage() {
        resolved_url = storage
            .upload_from_url(&resolved_url)
            .await
            .map_err(|err| {
                AppError::InternalServerError(format!("Failed to upload image: {}", err))
            })?;
    }

    Ok((resolved_url, notes, tags))
}

async fn finalize_cdn_status(
    state: &AppState,
    owner_user_id: Uuid,
    bucket_id: Uuid,
    image_id: Uuid,
    resolved_url: String,
) {
    // Async re-host Discord CDN URLs to B2 so they remain valid permanently.
    // We spawn off-thread so the caller is not delayed.
    if StorageService::is_discord_cdn(&resolved_url) {
        if let Some(storage) = state.storage().cloned() {
            let pool = state.pool.clone();
            let image_repo = state.image_repo.clone();
            let image_id_str = image_id.to_string();
            tokio::spawn(async move {
                match storage.upload_from_url(&resolved_url).await {
                    Ok(cdn_url) => {
                        let _ = sqlx::query(
                            "UPDATE images SET cdn_url = ?, cdn_status = 'migrated' WHERE id = ?",
                        )
                        .bind(&cdn_url)
                        .bind(&image_id_str)
                        .execute(&pool)
                        .await;
                        image_repo
                            .invalidate_image(owner_user_id, bucket_id, image_id)
                            .await;
                    }
                    Err(StorageError::FetchFailed(_)) => {
                        let _ = sqlx::query("UPDATE images SET cdn_status = 'broken' WHERE id = ?")
                            .bind(&image_id_str)
                            .execute(&pool)
                            .await;
                        image_repo
                            .invalidate_image(owner_user_id, bucket_id, image_id)
                            .await;
                    }
                    Err(e) => {
                        // UploadFailed — leave cdn_status as 'pending' for retry
                        tracing::warn!("CDN re-host upload failed for {}: {}", resolved_url, e);
                    }
                }
            });
        }
    } else {
        // Non-Discord URLs are already stable — mark as migrated immediately.
        let _ = sqlx::query("UPDATE images SET cdn_url = ?, cdn_status = 'migrated' WHERE id = ?")
            .bind(&resolved_url)
            .bind(image_id.to_string())
            .execute(&state.pool)
            .await;
    }
}

pub async fn create_image(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(bucket_id): Path<Uuid>,
    ValidatedJson(request): ValidatedJson<CreateImageRequest>,
) -> Result<Json<ImageResponse>, AppError> {
    let title = validate_title(request.title)?;
    let (resolved_url, auto_notes, social_tags) =
        resolve_and_upload_url(&state, &request.url).await?;
    let tags = merge_social_tags(request.tags.as_deref().unwrap_or_default(), &social_tags);

    let repo = state.image_repo.clone();
    let mut image = repo
        .create_with_metadata(
            user.user_id,
            bucket_id,
            &resolved_url,
            title.as_deref(),
            false,
            1,
            &tags,
        )
        .await?;

    if let Some(notes) = &auto_notes
        && repo
            .update_notes(user.user_id, bucket_id, image.id, Some(notes))
            .await
            .unwrap_or(false)
    {
        image.notes = Some(notes.clone());
    }

    finalize_cdn_status(&state, user.user_id, bucket_id, image.id, resolved_url).await;

    Ok(Json(image_response_from_stored(image, 0)))
}

pub async fn upload_image(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(bucket_id): Path<Uuid>,
    multipart: Multipart,
) -> Result<Json<ImageResponse>, AppError> {
    let is_root_admin = state.is_root_admin(user.user_id).await?;
    let has_permission = is_root_admin
        || state
            .admin_repo
            .has_permission(user.user_id, "upload_local_images")
            .await?;
    if !has_permission {
        return Err(AppError::Forbidden);
    }

    let bucket = state
        .bucket_repo
        .get_by_id(bucket_id)
        .await?
        .ok_or(AppError::Forbidden)?;
    if bucket.owner_user_id != user.user_id {
        return Err(AppError::Forbidden);
    }

    let bytes = read_uploaded_file_bytes(multipart).await?;
    let storage = state
        .storage()
        .ok_or_else(|| AppError::InternalServerError("storage is not configured".to_string()))?;
    let cdn_url = storage
        .upload_image_bytes(bytes)
        .await
        .map_err(map_upload_storage_error)?;

    let image = state
        .image_repo
        .create_with_metadata(user.user_id, bucket_id, &cdn_url, Some(""), false, 1, &[])
        .await?;

    Ok(Json(image_response_from_stored(image, 0)))
}

pub async fn delete_image(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((bucket_id, image_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let repo = state.image_repo.clone();
    let deleted = repo
        .delete_for_user(user.user_id, bucket_id, image_id)
        .await?;
    if deleted && let Some(storage) = state.storage() {
        let _ = storage.garbage_collect_orphaned_objects().await;
    }
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

pub async fn update_image(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((bucket_id, image_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateImageRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let repo = state.image_repo.clone();
    let existing = repo
        .get_for_owner(user.user_id, bucket_id, image_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Validate cheap fields first, before expensive I/O
    let title = match request.title {
        Some(title) => Some(validate_title(title)?),
        None => None,
    };
    let random_weight = request
        .random_weight
        .map(validate_random_weight)
        .transpose()?;

    // Now do expensive I/O
    let (resolved_url, auto_notes, auto_tags) = match &request.url {
        Some(new_url) => {
            let (url, notes, tags) = resolve_and_upload_url(&state, new_url).await?;
            (Some(url), notes, Some(tags))
        }
        None => (None, None, None),
    };

    let notes_patch = compute_notes_patch(request.notes, auto_notes, existing.notes.as_deref());

    let patch = UpdateImageMetadataPatch {
        title,
        notes: notes_patch,
        favorite: request.favorite,
        random_weight,
        tags: auto_tags
            .map(|social_tags| {
                merge_social_tags(request.tags.as_deref().unwrap_or_default(), &social_tags)
            })
            .or(request.tags),
        url: resolved_url.clone(),
    };

    let updated = repo
        .update_metadata_partial(user.user_id, bucket_id, image_id, &patch)
        .await?;

    if !updated {
        return Err(AppError::NotFound);
    }

    if let Some(resolved_url) = resolved_url {
        finalize_cdn_status(&state, user.user_id, bucket_id, image_id, resolved_url).await;
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

pub async fn bulk_update_images(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(bucket_id): Path<Uuid>,
    Json(request): Json<BulkUpdateImagesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if request.image_ids.is_empty() {
        return Err(AppError::BadRequest("imageIds is required".to_string()));
    }
    if request.image_ids.len() > 100 {
        return Err(AppError::BadRequest(
            "imageIds must include 100 images or fewer".to_string(),
        ));
    }

    let random_weight = request
        .random_weight
        .map(validate_random_weight)
        .transpose()?;
    let add_tags = normalized_tag_inputs(&request.add_tags);
    let remove_tags = normalized_tag_inputs(&request.remove_tags);

    if request.favorite.is_none()
        && random_weight.is_none()
        && add_tags.is_empty()
        && remove_tags.is_empty()
    {
        return Err(AppError::BadRequest(
            "at least one metadata change is required".to_string(),
        ));
    }

    let repo = state.image_repo.clone();
    let updated = repo
        .update_metadata_bulk(
            user.user_id,
            bucket_id,
            &BulkImageMetadataPatch {
                image_ids: request.image_ids,
                favorite: request.favorite,
                random_weight,
                add_tags,
                remove_tags,
            },
        )
        .await?;

    if updated == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "updated": updated })))
}

pub async fn bulk_delete_images(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(bucket_id): Path<Uuid>,
    Json(request): Json<BulkDeleteImagesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if request.image_ids.is_empty() {
        return Err(AppError::BadRequest("imageIds is required".to_string()));
    }
    if request.image_ids.len() > 100 {
        return Err(AppError::BadRequest(
            "imageIds must include 100 images or fewer".to_string(),
        ));
    }

    let repo = state.image_repo.clone();
    let deleted = repo
        .delete_bulk(user.user_id, bucket_id, &request.image_ids)
        .await?;
    if deleted > 0
        && let Some(storage) = state.storage()
    {
        let _ = storage.garbage_collect_orphaned_objects().await;
    }

    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

pub async fn bulk_move_images(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(bucket_id): Path<Uuid>,
    Json(request): Json<BulkMoveImagesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if request.image_ids.is_empty() {
        return Err(AppError::BadRequest("imageIds is required".to_string()));
    }
    if request.image_ids.len() > 100 {
        return Err(AppError::BadRequest(
            "imageIds must include 100 images or fewer".to_string(),
        ));
    }

    let repo = state.image_repo.clone();
    let moved = repo
        .move_bulk(
            user.user_id,
            bucket_id,
            &request.image_ids,
            request.new_bucket_id,
        )
        .await?;

    if moved == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "moved": moved })))
}

pub async fn move_image(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((bucket_id, image_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<MoveImageRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let repo = state.image_repo.clone();
    let updated = repo
        .move_to_bucket(user.user_id, bucket_id, image_id, request.new_bucket_id)
        .await?;

    if !updated {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

pub async fn record_image_send(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((bucket_id, image_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let repo = &state.send_history_repo;

    if repo
        .has_recent_send(user.user_id, image_id, PICKER_DEBOUNCE_WINDOW_SECS)
        .await?
    {
        return Ok(Json(serde_json::json!({ "recorded": false })));
    }

    let recent_count = repo
        .count_recent_by_visibility(user.user_id, "picker", PICKER_RATE_LIMIT_WINDOW_SECS)
        .await?;
    if recent_count >= PICKER_RATE_LIMIT_MAX {
        return Err(AppError::TooManyRequests);
    }

    match repo
        .record(user.user_id, bucket_id, image_id, "picker")
        .await
    {
        Ok(()) => Ok(Json(serde_json::json!({ "recorded": true }))),
        Err(sqlx::Error::RowNotFound) => Err(AppError::NotFound),
        Err(err) => Err(AppError::from(err)),
    }
}

pub async fn build_image_responses(
    pool: SqlitePool,
    requester_user_id: Option<Uuid>,
    images: Vec<StoredImage>,
) -> Result<Vec<ImageResponse>, sqlx::Error> {
    let send_counts = match requester_user_id {
        Some(requester_user_id) => {
            let send_history = SendHistoryRepository::new(pool);
            let image_ids = images.iter().map(|image| image.id).collect::<Vec<_>>();
            send_history
                .count_for_images(requester_user_id, &image_ids)
                .await?
        }
        None => std::collections::HashMap::new(),
    };

    Ok(images
        .into_iter()
        .map(|image| {
            let send_count = send_counts.get(&image.id).copied().unwrap_or(0);
            image_response_from_stored(image, send_count)
        })
        .collect())
}

pub fn image_response_from_stored(image: StoredImage, send_count: i64) -> ImageResponse {
    ImageResponse {
        id: image.id.to_string(),
        url: image
            .cdn_url
            .filter(|_| image.cdn_status.as_deref() == Some("migrated"))
            .unwrap_or(image.url),
        cdn_status: image.cdn_status,
        title: image.title,
        favorite: image.favorite,
        random_weight: image.random_weight,
        tags: image.tags,
        send_count,
        created_at: image.created_at,
        notes: image.notes,
    }
}

fn validate_title(title: Option<String>) -> Result<Option<String>, AppError> {
    match title {
        Some(title) if title.chars().count() > 200 => Err(AppError::BadRequest(
            "title must be 200 characters or fewer".to_string(),
        )),
        Some(title) => Ok(normalize_optional_text(Some(title))),
        None => Ok(None),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    match value {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(value),
        None => None,
    }
}

fn compute_notes_patch(
    explicit_notes: Option<Option<String>>,
    auto_notes: Option<String>,
    existing_notes: Option<&str>,
) -> Option<Option<String>> {
    if let Some(explicit) = explicit_notes {
        return Some(normalize_optional_text(explicit));
    }

    let auto = auto_notes?;
    let existing_is_empty = existing_notes.map(str::trim).is_none_or(str::is_empty);
    if existing_is_empty {
        Some(Some(auto))
    } else {
        None
    }
}

fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

fn validate_random_weight(random_weight: i64) -> Result<i64, AppError> {
    if (0..=10).contains(&random_weight) {
        Ok(random_weight)
    } else {
        Err(AppError::BadRequest(
            "randomWeight must be between 0 and 10".to_string(),
        ))
    }
}

fn parse_tag_filter(tags: Option<String>) -> Vec<String> {
    tags.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn merge_social_tags(manual_tags: &[String], social_tags: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    for tag in manual_tags.iter().chain(social_tags) {
        if !merged
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(tag))
        {
            merged.push(tag.clone());
        }
    }
    merged
}

fn normalized_tag_inputs(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for tag in tags {
        let cleaned = tag
            .trim()
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if cleaned.is_empty() {
            continue;
        }
        let folded = cleaned.to_lowercase();
        if seen.insert(folded) {
            normalized.push(cleaned);
        }
    }

    normalized
}

async fn read_uploaded_file_bytes(mut multipart: Multipart) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    {
        let mut field = multipart
            .next_field()
            .await
            .map_err(|err| AppError::BadRequest(format!("invalid multipart upload: {err}")))?
            .ok_or_else(|| {
                AppError::BadRequest("multipart field `file` is required".to_string())
            })?;

        if field.name() != Some("file") {
            return Err(AppError::BadRequest(
                "multipart field must be named `file`".to_string(),
            ));
        }

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|err| AppError::BadRequest(format!("invalid multipart upload: {err}")))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_LOCAL_UPLOAD_BYTES {
                return Err(AppError::PayloadTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
    }

    if multipart
        .next_field()
        .await
        .map_err(|err| AppError::BadRequest(format!("invalid multipart upload: {err}")))?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "only one multipart field is allowed".to_string(),
        ));
    }

    Ok(bytes)
}

fn map_upload_storage_error(error: StorageError) -> AppError {
    match error {
        StorageError::InvalidImage => AppError::BadRequest("invalid image data".to_string()),
        StorageError::UploadFailed(message) => AppError::InternalServerError(message),
        StorageError::FetchFailed(message) => AppError::InternalServerError(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_stored_image(
        url: &str,
        cdn_url: Option<&str>,
        cdn_status: Option<&str>,
    ) -> StoredImage {
        StoredImage {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            bucket_id: Uuid::new_v4(),
            url: url.to_string(),
            cdn_url: cdn_url.map(str::to_string),
            cdn_status: cdn_status.map(str::to_string),
            title: None,
            favorite: false,
            random_weight: 1,
            tags: vec![],
            created_at: "2026-06-28T00:00:00Z".to_string(),
            notes: None,
        }
    }

    #[test]
    fn url_swap_migrated_status_returns_cdn_url() {
        let original = "https://cdn.discordapp.com/attachments/1/2/img.png";
        let cdn = "https://media.memebucket.app/abc123.webp";
        let image = make_stored_image(original, Some(cdn), Some("migrated"));
        let response = image_response_from_stored(image, 0);
        assert_eq!(response.url, cdn);
        assert_eq!(response.cdn_status.as_deref(), Some("migrated"));
    }

    #[test]
    fn url_swap_pending_status_returns_original_url() {
        let original = "https://cdn.discordapp.com/attachments/1/2/img.png";
        let image = make_stored_image(original, None, Some("pending"));
        let response = image_response_from_stored(image, 0);
        assert_eq!(response.url, original);
        assert_eq!(response.cdn_status.as_deref(), Some("pending"));
    }

    #[test]
    fn url_swap_broken_status_returns_original_url() {
        let original = "https://cdn.discordapp.com/attachments/1/2/img.png";
        let image = make_stored_image(original, None, Some("broken"));
        let response = image_response_from_stored(image, 0);
        assert_eq!(response.url, original);
        assert_eq!(response.cdn_status.as_deref(), Some("broken"));
    }

    #[test]
    fn url_swap_migrated_without_cdn_url_falls_back_to_original() {
        // cdn_url is None even though cdn_status is "migrated" — defensive fallback
        let original = "https://example.com/img.gif";
        let image = make_stored_image(original, None, Some("migrated"));
        let response = image_response_from_stored(image, 0);
        assert_eq!(response.url, original);
    }

    #[test]
    fn url_swap_no_cdn_status_returns_original_url() {
        let original = "https://example.com/img.gif";
        let image = make_stored_image(original, None, None);
        let response = image_response_from_stored(image, 0);
        assert_eq!(response.url, original);
    }

    #[test]
    fn merge_social_tags_preserves_manual_tags_and_first_seen_spelling() {
        let manual = vec!["Manual".to_string(), "funny".to_string()];
        let social = vec!["FUNNY".to_string(), "Bluesky".to_string()];

        assert_eq!(
            merge_social_tags(&manual, &social),
            vec!["Manual", "funny", "Bluesky"]
        );
    }

    #[test]
    fn compute_notes_patch_explicit_notes_always_wins() {
        // Explicit notes present, even though auto-fill notes are also available and existing notes are empty.
        let result = compute_notes_patch(
            Some(Some("My own note".to_string())),
            Some("@handle: tweet text".to_string()),
            None,
        );
        assert_eq!(result, Some(Some("My own note".to_string())));
    }

    #[test]
    fn compute_notes_patch_explicit_null_clears_notes_even_with_auto_available() {
        let result = compute_notes_patch(
            Some(None),
            Some("@handle: tweet text".to_string()),
            Some("existing notes"),
        );
        assert_eq!(result, Some(None));
    }

    #[test]
    fn compute_notes_patch_auto_fills_when_existing_notes_are_empty() {
        let result = compute_notes_patch(None, Some("@handle: tweet text".to_string()), None);
        assert_eq!(result, Some(Some("@handle: tweet text".to_string())));
    }

    #[test]
    fn compute_notes_patch_auto_fills_when_existing_notes_are_whitespace_only() {
        let result =
            compute_notes_patch(None, Some("@handle: tweet text".to_string()), Some("   "));
        assert_eq!(result, Some(Some("@handle: tweet text".to_string())));
    }

    #[test]
    fn compute_notes_patch_does_not_overwrite_existing_notes() {
        let result = compute_notes_patch(
            None,
            Some("@handle: tweet text".to_string()),
            Some("Already have notes here"),
        );
        assert_eq!(result, None);
    }

    #[test]
    fn compute_notes_patch_no_change_when_no_auto_notes_available() {
        let result = compute_notes_patch(None, None, None);
        assert_eq!(result, None);
    }

    #[test]
    fn map_upload_storage_error_maps_invalid_image_to_bad_request() {
        let error = map_upload_storage_error(StorageError::InvalidImage);
        assert!(matches!(error, AppError::BadRequest(message) if message == "invalid image data"));
    }

    #[test]
    fn map_upload_storage_error_keeps_other_upload_failures_as_server_errors() {
        let error =
            map_upload_storage_error(StorageError::UploadFailed("b2 write failed".to_string()));
        assert!(
            matches!(error, AppError::InternalServerError(message) if message == "b2 write failed")
        );
    }
}
