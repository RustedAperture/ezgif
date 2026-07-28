# Changelog

## v0.2.10 - Jul 17, 2026

### Added

- Added dedicated 404, 429, and 500 error pages with recovery actions.
- Selecting an image in the Picker (web and desktop) now counts toward that image's send count, matching Discord sends.
- Pasting a link into the Picker's or bucket page's bulk-add links box now automatically starts a new line, so you can paste the next link right away.
- The Picker's bulk-add links now shows which specific links failed and why, matching the bucket page's existing failed-imports list.
- Closing the Bucket Settings dialog while renaming a bucket now warns about unsaved changes instead of silently discarding your edit.

### Fixed

- Fixed moving an image to another bucket (via drag-and-drop or the bucket dropdown in the image dialog) failing with a 422 error.
- Pasting a Klipy gif/sticker/clip page link (e.g. klipy.com/gifs/...) now shows a clear message explaining it can't be imported directly, instead of a generic fetch error.
- Fixed the website hiding the Add Image / Search GIFs / Bulk Add Links toolbar on your own Inbox bucket, matching the Picker's existing support for adding links to Inbox.
- Fixed the Bulk Add Links dialog growing wider than the screen when a very long link was being processed.
- Aligned the bucket image selection toolbar radius with the image cards.
- Fixed Enter not submitting several image and bucket editors; Bulk Add Links still uses Enter for new lines.

## v0.2.9 - Jul 15, 2026

### Fixed

- Fixed the Picker's **Add links** screen showing a duplicate bucket dropdown; it now uses the top selector only.
- Fixed the Picker's links textarea zooming in on focus on mobile browsers by using the standard input text size.
- Fixed the Picker's default-selected result card showing a border that could bleed into the next masonry column; the selection ring now only appears once you navigate with the keyboard.
- Fixed the Picker's "New update available" banner overflowing past the right edge; it now aligns with the search and bucket rows above it.
- Improved the security of server-side media URL fetching (link imports and video conversion).
- Improved the security of the desktop app's connection to self-hosted servers.
- Fixed the Picker's changelog banner losing its gap from the image grid once you scroll.

### Changed

- Aligned Picker form controls (search, bucket selector, links textarea) with the app's default rounding and sizing for a more consistent look.
- Aligned Picker result cards with Library image cards, matching border, radius, hover, and selection styling.

## v0.2.8 - Jul 14, 2026

### Added

- Added **Bluesky link support**: pasting a Bluesky post now extracts its image or HLS video, converts videos to WebP, and saves the author's text as notes.
- Added **social hashtag import**: hashtags from X/Twitter and Bluesky posts are imported as image tags while preserving manually supplied tags.
- Added virtual **All** bucket.
- Added the thumbnail size slider to the Library so search results can be resized like bucket images.
- Added the ability to add multiple media links at once from the Picker using one link per line, with remembered bucket selection and a completion summary.

### Fixed

- Fixed **Tenor page-link resolution** so `/view/...` links correctly resolve their media image.

### Changed

- Adjusted the dashboard layout to use a centered 66% content column at the 2xl breakpoint and wider, while keeping smaller screens responsive.
- Creating a bucket immediately switches to it.
- Pinned **All**, **Favorites**, and **Inbox** at the top.
- Metadata edits, favorites, deletes, moves, and bulk actions work from system/aggregate views using each image’s real source bucket.
- Simplified Library image cards by removing weight and send counts, adding corner favorite controls, and using icon-only Copy and Open actions.
- Aligned Library image details with bucket details, including link editing and link actions.
- Library cards no longer display media URLs as fallback titles.

## v0.2.7 - Jul 13, 2026

### Added

- Added **X/Twitter link support**: pasting a link to an X (formerly Twitter) post now extracts the attached photo, video, or GIF, converts it to WebP, and uploads it to your bucket — the same as any other image URL.
- Added the ability to **edit an image's link**: open any image's details in the dashboard, click Edit, and replace its URL alongside the rest of its details. The new link goes through the same extraction/conversion pipeline as adding a fresh image (including X/Twitter links), while keeping the image's title, tags, notes, and favorite status intact.
- Added a **changelog update notice** to the Picker: when a new changelog entry is published, the Picker shows a small banner linking to it, so desktop users don't have to check the website manually.
- Added **automatic post credit**: adding an image from an X/Twitter link now fills in its notes with the author's handle and the post text (e.g. `@handle: post text`), unless the image already has notes.

## v0.2.6 - Jul 7, 2026

### Fixed

- Fixed the Picker popup appearing on screen every time the app launches (including automatically at login via **Launch at startup**); it now stays hidden until summoned via the tray icon or the global hotkey.

## v0.2.5 - Jun 29, 2026

### Fixed

- Fixed **Restart to update** not doing anything when clicked: the app now relaunches automatically after the update installs instead of silently installing in the background.

## v0.2.4 - Jun 29, 2026

### Added

- Added a **Settings window** accessible from the tray menu with fields for Server URL, global hotkey, and Launch at startup. Click "Settings…" in the tray to open it.
- Added **live hotkey capture** in the Settings window: click the Hotkey field and press any key combination to set a new global shortcut. The new hotkey takes effect immediately on save.

### Changed

- Moved the **Launch at startup** toggle from the tray menu into the Settings window.

### Fixed

- Fixed a startup crash when a saved custom hotkey was already in use by another app: the app now falls back to the default hotkey (`CmdOrCtrl+Shift+M`) instead of panicking.
- Fixed the Settings window red close button destroying the window instead of hiding it; the window is now always hidden on close so it can be reopened from the tray.
- Fixed the hotkey recorder accepting `Shift`-only combos (e.g. `Shift+A`) that would hijack normal typing; a non-Shift modifier (Cmd, Ctrl, or Alt) is now required.

## v0.2.3 - Jun 28, 2026

### Added

- Added **B2 content deduplication**: uploaded media is now hashed with BLAKE3 after conversion, and identical content is stored only once in B2 regardless of how many users upload it. Subsequent uploads of the same image return the existing CDN URL instantly without re-uploading.
- Added **mobile instructions** to the Download page: Android and iOS users are shown platform-specific steps to add the Picker as a home screen shortcut (no install required). Desktop visitors also see a side-by-side iOS/Android guide card.

## v0.2.2 - Jun 28, 2026

### Added

- Added a **drag bar** to the Picker overlay with a four-arrow move icon, close button, and minimize button. Drag the bar to reposition the window; position is saved automatically after each move.

## v0.2.1 - Jun 28, 2026

### Added

- Added **launch at startup** toggle to the Picker tray menu: right-click the tray icon and click "Launch at startup" to toggle; a checkmark (✓) indicates when it's enabled.
- Added **draggable window** to the Picker: drag by the top strip to reposition the overlay anywhere on screen. Position is saved and restored on next open.
- Added **screen-aware positioning**: the Picker now clamps to monitor bounds when summoned via hotkey, so it never opens partially off-screen. If a previously saved position is no longer visible (e.g. after unplugging a monitor), it falls back to cursor-relative positioning.
- Added **auto-update** to the Picker: the app checks for new releases in the background on startup. When an update is available, a "Restart to update (vX.Y.Z)" item appears in the tray menu; click it to download and apply the update.

## v0.2.0 - Jun 28, 2026

### Added

- Added **Media Permanence**: Discord CDN URLs (which expire) are now automatically re-hosted to Backblaze B2 cloud storage and served through Cloudflare, making image and GIF links permanent.
- Added background migration job that runs at server startup to re-host any existing images still pointing to Discord CDN.
- Added broken-link placeholder in the web dashboard and Picker: images whose source URL was already expired when we tried to fetch them display a clear "⚠ Link unavailable" indicator instead of a broken image icon.

## v0.1.9 - Jun 27, 2026

### Added

- Added **Telegram Login** as a second authentication provider. Users can now sign in with Telegram alongside Discord.
- Added multi-provider identity model: one account can have multiple login methods (Discord and/or Telegram) linked to it.
- Added **Connected Accounts** section to account settings for viewing and unlinking auth providers.
- Added role-based access control (`role` column on users) for future admin panel support.

## v0.1.8 - Jun 26, 2026

### Added

- Added the **memeBucket Picker** desktop app (Tauri): a lightweight overlay that lives in the macOS menu bar or Windows/Linux system tray and never occupies the dock or taskbar.
- Added a global hotkey (`⌘ Shift M` / `Ctrl Shift M`) to summon and dismiss the Picker from any app.
- Added a `/picker` web page powering the overlay: masonry image grid with live search, bucket filter, keyboard navigation (↑↓ for linear, ←→ for column-aware jumping), and Enter-to-paste.
- Added a `/download` page that fetches the latest GitHub release via the API, auto-detects the visitor's OS, and shows a direct download button with file size alongside a Getting Started guide.
- Added a Download card to the dashboard and a Download link to the navigation bar.
- Added a GitHub Actions release workflow (`release-desktop.yml`) that builds and publishes signed-free binaries for macOS Apple Silicon, macOS Intel, Windows, and Linux on `desktop-v*` tag pushes.

### Changed

- Improved the Picker selection highlight to use a solid full-opacity ring with an offset gap for much higher contrast.
- Removed the item count badge from the Picker header to reduce visual noise.

## v0.1.7 - Jun 26, 2026

### Added

- Added a dedicated, thread-safe in-memory caching layer using `moka` in the backend to cache read-heavy database lookups (bucket lists, names, subscriptions, whitelists, and image lists) with precise write-time invalidation hooks.
- Added declarative request payload validation at the API boundary using the `validator` crate and a custom Axum extractor `ValidatedJson`.
- Added non-blocking asynchronous external process execution in `video_converter.rs` utilizing `tokio::process::Command` to prevent blocking worker threads during `ffmpeg` conversions.
- Added a `delete` method to the `UserRepo` trait and shifted the complex bulk database import/export transaction logic entirely to the repository layer, rendering the service layer fully decoupled and database-agnostic.
- Migrated the global frontend session and authentication state from React Context to a `zustand` store, optimizing component re-renders.

### Changed

- Refactored server startup to bind and run the HTTP listener first, scheduling Discord command registration asynchronously in a background task.
- Refactored `AccountService` to consume repository traits via Dependency Injection instead of holding a concrete `SqlitePool` and executing raw SQL queries.

## v0.1.6 - Jun 25, 2026

### Added

- Added bulk editing features to the bucket view, including **Bulk Delete** and **Bulk Move** with efficient transactional backend endpoints.
- Added **Bulk Add Links** allowing users to paste multiple image/video URLs at once with a step-by-step progress UI and error recovery (pre-filling with only failed URLs for easy retries).
- Added batch actions to **Select All** and **Copy Links** for all selected items.
- Added **Import Data** functionality in the Account Settings menu, allowing users to upload their exported backup JSON files to easily restore or duplicate their buckets and media with all metadata and tags fully preserved.
- Added client-side format validation and server-side duplicate prevention to ensure seamless imports.

### Changed

- Rebranded the Discord system bucket from `"Added from Discord"` to `"Inbox"`, including a database migration to automatically migrate existing records.
- Removed redundant drag count badges from selected cards for a cleaner visual aesthetic.

### Fixed

- Fixed the "Export Data" functionality which was failing due to outdated database table references (`pools`) from the rebranding.
- Updated the export data payload to include all newly added image metadata fields (title, favorite, random weight, tags, notes, and created_at).

## v0.1.5 - Jun 23, 2026

### Added

- Added mobile-specific touch gestures: single tap to copy media URLs to the clipboard, and long press (500ms) to open the detailed image properties modal in both the Bucket and Search views.
- Enabled clicking images on desktop viewports in the Search page to open the properties dialog, matching the Bucket view behavior.

### Changed

- Rebranded the entire application from **ezGif** to **memeBucket**, renaming all references to **pools** to **buckets** throughout the database, backend services, Discord commands, and frontend UI.
- Refactored the Bucket control toolbar to use a responsive two-row layout on mobile viewports, allowing the size slider to remain fully functional while the input box stretches to fill the screen width.
- Configured Sonner toast notifications to dynamically display at the top center of the screen on mobile devices.

## v0.1.4 - Jun 22, 2026

### Added

- Added an optional `target` parameter to the `/mb` slash command to send the GIF directly to a specific user.
- Added a "Reply with GIF" right-click message context menu command to instantly send a GIF directed at the author of the selected message.

### Fixed

- Fixed an issue causing right-click "Reply with GIF" modals to time out due to slow database reads by enabling SQLite WAL mode.
- Fixed a Discord API `400 Bad Request` error preventing the modal from opening by removing unsupported select menus.
- Added a friendly error message listing available buckets if an invalid bucket name is entered in the modal.

### Changed

- Updated Discord integration to embed GIFs so URLs are hidden instead of using zero-width spaces.
- Restored the user's specific Discord profile accent color to embeds sent from the bot.
- Updated dependencies to patch security vulnerabilities.

## v0.1.3 - Jun 14, 2026

### Added

- Added Library search for saved GIFs and images across accessible buckets, with filters for tags, bucket, favorites, and random-enabled state.
- Added image metadata fields for title, tags, favorite status, random weight, and notes.
- Added metadata editing from image details and bulk editing for selected images.
- Added Klipy metadata suggestions so saved GIFs can start with a title and suggested tags.
- Added a Library card to the dashboard.
- Added a "Disable usage" toggle to image buckets.
- Auto-injected a read-only "Favorites" bucket containing all starred media.
- Expanded the "Paste URL" field to natively double as a Klipy GIF search query.
- Added a star when hovering over an image to easily toggle favorite status.

### Changed

- Improved random image selection with per-image weights and stronger recent-repeat avoidance.
- Renamed the global saved-media search surface to Library to distinguish it from searching Klipy for new GIFs.
- Expanded access checks and tests for library search across owned, subscribed, public, private, and whitelisted buckets.
- Refactored bucket view and search pages to share a unified responsive layout.
- System buckets (like Favorites or Inbox) are now automatically hidden when empty.

## v0.1.2 - Jun 11, 2026

### Added

- Added an "Add to Bucket" Discord message context menu command to save images directly from messages into an "Inbox" bucket.
- Added the ability to rename image buckets.

### Changed

- Migrated the web dashboard's sidebar layout to use standard Shadcn UI components.
- Consolidated bucket settings (rename, delete, unsubscribe) into a clean Settings modal.

## v0.1.1 - Jun 11, 2026

### Added

- Added homepage buttons for Ko-fi support and the GitHub repository.
- Added a footer theme selector with Light, Dark, and Auto modes.
- Added drag-and-drop support and a modal dropdown for moving images between buckets.
- Added a GIF search feature powered by the Klipy API, accessible directly from the bucket image form.

### Fixed

- Fixed theme selector styling so only one mode appears selected at a time.
- Fixed an issue in GIF search where "Load more" would append duplicate results.

### Changed

- Improved the GIF search layout by using a masonry-style columns layout to better preserve image aspect ratios.

## v0.1.0 - Jun 10, 2026

### Added

- Initial memeBucket Discord user app and web dashboard.
- Discord OAuth sign-in and session-backed account management.
- Personal media buckets for organizing image and GIF URLs.
- Discord commands for creating buckets, adding images, listing buckets, opening the dashboard, and sending a random image from a selected bucket.
- Web dashboard for managing buckets, images, notes, and account username.
- Bucket sharing with share links, subscriptions, subscriber counts, and optional whitelist access.
- Account export endpoint for owned buckets and image URLs.
- Account deletion endpoint for removing account-linked data.
- Terms of Service, Privacy Policy, Changelog, and GPLv3 License pages.

### Privacy and Security

- Discord user identity is stored as an HMAC-SHA256 user key rather than a raw Discord user ID.
- State-changing dashboard requests use CSRF protection.
- Selected routes use rate limiting.
