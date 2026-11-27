# Git-Powered In-App Auto-Update Blueprint

> **Agent Instructions**: Read this entire specification before writing any code. This document defines a production-hardened, seamless in-app auto-updater powered by Git tags and GitHub Releases (`Auto Check -> Detect Priority -> Modular UI -> Verified Background Download -> Silent Install -> Auto Restart`).
>
> **CRITICAL**: Before implementing, you MUST ask the user the **Pre-Implementation Discovery Questions** in Section 1 to lock in their desired UI style, copy tone, and repository parameters. Do not guess UI preferences.

---

## End-to-End User Experience Flow

1. **Auto-Check on Startup (and Manual Check)**: App silently queries the latest published release metadata on launch (offline-resilient, falling back to cached update metadata only when offline).
2. **Priority Classification**: The updater inspects the release tag and metadata to classify the update as either **Standard (Non-Blocking)** or **Critical (Blocking)**.
3. **Modular UI Presentation**: Displays the user-chosen UI component and text style for Standard updates, or enforces the non-dismissible Blocking UI if flagged critical.
4. **In-App Background Download**: Clicking the Update button downloads the new installer directly in the background inside the app with a progress bar showing `"Downloading v<version>..."`—no manual browser downloads or setup wizard clicks required.
5. **Verified Silent Install & Auto-Restart**: Verifies file integrity, executes the installer silently (which terminates the running instance cleanly if needed), installs the update, and automatically relaunches the updated application.

> **UX Copy Golden Rule**: Never mention "GitHub", servers, or technical transfer details in end-user UI copy. Keep all messages short enough that they never truncate with `...`, and never write redundant instructions like *"Click Update Now to install..."* when an Update button is already right in front of the user.

---

## 1. Mandatory Pre-Implementation Discovery Questions

Ask the user these questions **first** and wait for their answers before writing code:

### Question 1: Standard (Non-Blocking) Update UI & Text Style
Which UI presentation and text style do you want for normal updates?

1. **Toast Notification / Top Banner (Non-blocking)**:
   - **(i) Minimal & Clean**: `"A new version is available. Update to install."` (with Update button and dismiss `X`).
   - **(ii) Value-Focused (1–2 concise lines)**: `"Update available with performance improvements and bug fixes."` (or parsed concise highlights from release notes).
2. **Modal Dialog (Non-blocking Feature Highlights)**:
   - Best when you want to highlight why the user should update before restarting.
   - Displays a concise **Headline**, **Value/Highlight Body**, and two buttons: **`Update now`** (Primary) and **`Remind me later`** (Secondary).

### Question 2: Forced / Critical Updates (Blocking) UI Style *(Required Capability)*
Forced/blocking support is mandatory so you can push urgent security or breaking fixes via git tags. When a **Blocking** update is detected, which style should be shown?

1. **Fixed Non-Closable Popup Modal**: Displays over a dimmed/blurred backdrop. Underlying app content remains visible behind the modal, but all background interaction is blocked and the modal cannot be closed.
2. **Full-Page Blocker View**: Covers the entire application window so underlying app content is completely hidden; the only available action is updating.

*(Both styles use the clean copy — Headline: **`"Update Required"`**, Body: **`"To continue using [App Name], please update to the latest version. This update includes essential security and stability improvements."`**, and a primary Update button.)*

### Question 3: Download Progress Display
While the progress bar moves during download:
- Show **only** `"Downloading v<version>..."` (do you also want a compact `%` badge next to it, or strictly just the text + progress bar?).

### Question 4: Project & Distribution Context
Please confirm:
- **App Name** (for `[App Name]` placeholders)
- **Source Repository (`OWNER/REPO`)** *(Tip: You can keep source code in a private repo and publish releases to a separate public repo via a PAT in CI—point the app's updater URL to the public release repo!)*
- **Tech Stack & Installer Format** (e.g., Tauri/Electron/Native, NSIS `/S`, Inno Setup `/VERYSILENT`, macOS `.dmg`, Linux `.AppImage`)

---

## 2. Modular Architecture Requirement

Design the frontend and backend cleanly so the user can change **UI style** or **UI text** later in **one centralized config file** without modifying updater logic:

### `updateConfig` Module (Single Source of Truth)
Expose typed configuration constants/objects containing:
- `standardUiStyle`: `'banner' | 'toast' | 'modal'`
- `standardCopyMode`: `'simple' | 'value'`
- `blockingUiStyle`: `'modal' | 'fullpage'`
- `copy` dictionary:
  - `standardHeadline(version)`
  - `standardSimpleBody`
  - `standardValueFallback`
  - `featureModalHeadline(version)`
  - `blockingHeadline`: `"Update Required"`
  - `blockingBody`: `"To continue using [App Name], please update to the latest version. This update includes essential security and stability improvements."`
  - `downloadingLabel(version)`: `` `Downloading v${version}...` ``
  - `updateButtonLabel`: `"Update"` / `"Update now"`
  - `laterButtonLabel`: `"Remind me later"`

Build the UI renderer polymorphically on `update.priority` (`'standard' | 'blocking'`) + `updateConfig`, ensuring a blocking update **cannot** be dismissed via `dismiss()`, `Escape`, or backdrop click.

---

## 3. Git Tag Priority Auto-Detection Logic

The developer controls whether a release is **Standard** or **Blocking** purely at release time using Git tags (or optional release note markers), without shipping app code changes:

### Tag Convention
- **Standard Update (Non-blocking)**:
  ```bash
  git tag v1.2.0
  git push origin v1.2.0
  ```
- **Critical / Forced Update (Blocking)**:
  ```bash
  git tag v1.2.0-critical
  git push origin v1.2.0-critical
  ```
  *(Support `-critical`, `-blocking`, `-force`, `+critical`, `+blocking`, and fallback release title/body markers `[critical]` / `[blocking]`.)*

### Parsing Rules
1. **SemVer Comparison**: `parse_semver(tag)` MUST strip leading `v`/`V` and split on `-`/`+` before parsing `(major, minor, patch)`, so `v1.2.0-critical` cleanly compares as `(1, 2, 0) > (1, 1, 9)`.
2. **Clean Version Display**: Strip priority suffixes (`-critical`, `-blocking`, `-force`) when storing `PendingUpdate.version` so users see clean `"Downloading v1.2.0..."`.
3. **Keep Manifests Standard SemVer**: Keep `Cargo.toml`, `package.json`, `tauri.conf.json`, and installer scripts at plain `1.2.0`. Only the **Git tag** carries `-critical` / `-blocking`.
4. **CI Compatibility**: In `.github/workflows/release.yml`, trigger on `push: tags: ['v*']` so both `v1.2.0` and `v1.2.0-critical` build and attach the installer asset automatically.

---

## 4. Security, Memory & Reliability Standards

Every implementation MUST satisfy these safeguards to avoid common updater loopholes:

### A. Pinned Repository & Protocol Security
1. **Single Repository Constant**: Define `GITHUB_REPO = "OWNER/REPO"` once and derive **both** the API metadata URL and the download URL allowlist from it so they can never drift out of sync.
2. **Strict Initial URL Allowlist**: Require `download_url` to start with `https://github.com/<OWNER>/<REPO>/releases/download/` and reject path traversal (`..`). **Never** whitelist redirect CDN hosts (e.g., `objects.githubusercontent.com`) as initial download URLs, or attackers could point IPC calls at arbitrary repositories.
3. **TLS & Redirect Hardening**: Force HTTPS-only for both initial requests and HTTP 302 redirects (`--proto =https --proto-redir =https --tlsv1.2` in curl, or strict HTTPS client config), and fail on HTTP error codes (`-f` / non-2xx).

### B. Concurrency, Timeouts & Integrity
4. **Concurrency Lock**: Guard the update download with an atomic boolean (`UPDATE_IN_PROGRESS`) and an RAII drop/finally reset so double-clicking Update cannot spawn competing downloads racing on the same temporary files.
5. **Stall & Connection Timeouts**: Configure connect timeouts (e.g., 15s) and minimum transfer speed timeouts (e.g., abort if stalled below 1 KB/s for 30s) so dropped Wi-Fi never leaves zombie background download threads or spinners hanging forever.
6. **Isolated Staging & Cleanup**: Download into an app-owned staging subfolder (e.g., `%LOCALAPPDATA%/[AppName]/updates/Update-<pid>.tmp`) rather than the global shared temp folder, and run a cleanup pass at app startup to delete leftover installers from previous updates.
7. **Strict Pre-Execution Verification**:
   - Explicitly handle missing `metadata()` errors (never silently skip validation).
   - Enforce minimum plausible installer size (e.g., `>= 1–5 MB` depending on app).
   - Verify exact byte match against HTTP `Content-Length` (when present).
   - Verify executable binary signature/magic bytes (e.g., `MZ` header on Windows PE executables) and optional SHA-256 checksum verification before renaming `.tmp` &rarr; `.exe` and executing.

### C. Cache Consistency & Restart Flow
8. **Fresh Priority Discovery on Startup**: Even on silent startup checks (`force = false`), query the remote release endpoint first so a newly pushed `-critical` update is never hidden behind an older cached `Standard` `pending_update`. Only fall back to the cached `pending_update` when offline/unreachable.
9. **Atomic Settings Persistence**: Persist settings via write-to-temp + atomic rename so abrupt restarts during installation never corrupt user preferences.
10. **Clean Silent Install & Auto-Relaunch**:
    - Emit `done: true` -> wait briefly (`~300–500 ms`) so UI shows completion -> spawn silent installer (`/S` on NSIS) detached -> exit current app process cleanly (`exit(0)`).
    - Installer stops any remaining background daemon processes (if multi-process), replaces binaries, and launches the freshly installed executable (`Exec '"$INSTDIR\App.exe"'`).
    - On transient download failure, show the inline error and allow in-app retry rather than unexpectedly forcing open a browser window.
