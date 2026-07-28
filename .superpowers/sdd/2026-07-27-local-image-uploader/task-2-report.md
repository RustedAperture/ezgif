# Task 2 report — local image uploader server upload path

Date: July 28, 2026

Files changed:

- `Cargo.toml`
- `Cargo.lock`
- `apps/server/src/services/storage.rs`
- `apps/server/src/api/images.rs`
- `apps/server/src/error.rs`
- `apps/server/src/router.rs`
- `apps/server/tests/images_api.rs`

What changed:

- Added `StorageService::upload_image_bytes(bytes: Vec<u8>) -> Result<String, StorageError>` and covered it with test-first storage cases for PNG-to-WebP conversion and duplicate dedup reuse.
- Added authenticated multipart upload handling at `POST /api/buckets/{bucket_id}/images/upload`.
- Required the multipart field name `file`.
- Checked `upload_local_images` via `AdminRepository::has_permission`.
- Enforced bucket ownership before accepting the upload.
- Kept the original file-size limit at 20 MiB and returned `413 Payload Too Large` from the explicit file-size check.
- Scoped a larger multipart body limit to only the upload route so `20 MiB + multipart overhead` reaches the handler without raising the global API limit.
- Decoded image bytes, converted accepted uploads to WebP, reused the existing BLAKE3/content-addressed storage path, and created separate image rows for duplicate uploads.
- Left the existing URL-import path unchanged.

Tests and results:

- `cargo test -p memebucket-server upload_image_`
  - Passed
  - Included:
    - `upload_image_permitted_owner_creates_image_row`
    - `upload_image_duplicate_upload_creates_second_row_with_same_url`
    - `upload_image_without_permission_returns_forbidden`
    - `upload_image_non_owner_returns_forbidden`
    - `upload_image_rejects_payload_over_twenty_mib`
    - `upload_image_rejects_malformed_image_bytes_without_creating_row`
- `cargo test -p memebucket-server`
  - Passed
- `cargo fmt --all --check`
  - Passed

Concerns:

- The upload route now uses a route-local body cap of 21 MiB so a 20 MiB file plus multipart framing reaches the handler. If request metadata grows materially in the future, that overhead budget may need a small adjustment.
- Invalid uploaded image bytes currently surface as `400 Bad Request`, which matches the focused tests. If product wants `422 Unprocessable Entity` instead, the handler mapping can be adjusted without changing the storage path.
