# Migration Plan: Tauri v2 + Svelte 5 + Tailwind for FileUC Settings

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Iced settings interface in `crates/fileuc-settings` with an on-demand Tauri v2 application using Svelte 5 and Tailwind CSS, guaranteeing pixel-perfect alignment, zero UI inconsistency, modern reactive components, and 0 MB idle webview overhead.

**Architecture:** A single unified executable (`fileuc.exe`) where the background core runs the low-level Windows mouse hook, Win32 layered Donut HUD (`tiny-skia`), and system tray icon at < 8 MB RAM. When the Settings UI is opened, `fileuc.exe` launches the Tauri v2 settings window on demand (`fileuc.exe --settings`). Svelte 5 + Tailwind assets are precompiled and embedded into the binary. When closed, WebView2 completely terminates, restoring web memory to 0 MB.

**Tech Stack:** 
- Backend: Rust, Tauri v2 (`tauri = "2"`, `tauri-build = "2"`), Windows API (`windows` crate), Named Pipe IPC (`FileUC_Ipc_Pipe`).
- Frontend: Svelte 5 (Runes `$state`), Tailwind CSS, Lucide icons (`lucide-svelte`), Vite.

---

## 1. System Architecture & Process Boundaries

```
+-----------------------------------------------------------------------------------+
|                                   fileuc.exe                                      |
|                                                                                   |
|  [Background Core] (Always Running, < 8 MB RAM)                                   |
|  - Windows Low-Level Mouse Hook (fileuc-hook)                                     |
|  - Donut HUD Ring: Win32 Layered Window (UpdateLayeredWindow) + tiny-skia         |
|  - System Tray Icon & Single-Instance Mutex (FileUC_SingleInstance_Mutex)         |
|  - Conversion Dispatcher (fileuc-convert)                                         |
|  - Named Pipe IPC Server (FileUC_Ipc_Pipe)                                        |
|                                                                                   |
|  [On-Demand Settings Subsystem] (Launched via `fileuc.exe --settings`)            |
|  - Tauri v2 Window Manager (Windows Edge WebView2)                                |
|  - Precompiled Embedded Frontend (Svelte 5 + Tailwind CSS + Lucide Icons)          |
|  - Native Rust Tauri Commands:                                                    |
|      * `get_settings()` -> AppSettings                                            |
|      * `save_settings(settings)` -> syncs via FileUC_Ipc_Pipe + SettingsStore     |
|      * `toggle_service(enabled)` -> syncs service state to daemon                 |
|      * `get_plugin_status()` -> { ffmpeg: PluginInfo, docx: PluginInfo }          |
|      * `install_plugin(name)` / `uninstall_plugin(name)`                          |
|  - Exit Behavior: Closing window terminates Settings process; 0 MB webview RAM    |
+-----------------------------------------------------------------------------------+
```

---

## 2. Directory Structure & File Map

```
crates/
├── fileuc-settings/
│   ├── Cargo.toml                  # Tauri v2 + serde dependencies
│   ├── build.rs                    # tauri_build::build() to embed UI
│   ├── tauri.conf.json             # Tauri window size (800x560), title, permissions
│   ├── src/
│   │   ├── lib.rs                  # Exports SettingsController & run_tauri()
│   │   ├── commands.rs             # Tauri #[tauri::command] handlers
│   │   ├── store.rs                # AppSettings persistence (settings.json)
│   │   └── plugin_manager.rs       # FFmpeg & docx installer with download callbacks
│   └── ui/                         # Svelte 5 + Tailwind Frontend
│       ├── package.json            # svelte 5, tailwindcss, lucide-svelte, vite
│       ├── vite.config.ts          # Vite build config outputting to dist/
│       ├── svelte.config.js        # Svelte compiler config
│       ├── tailwind.config.js      # Custom theme colors matching Donut HUD
│       ├── index.html              # Shell HTML
│       └── src/
│           ├── app.css             # Tailwind directives & CSS variables
│           ├── main.ts             # App mounting
│           ├── App.svelte          # Shell layout (Sidebar + Content Scrollable)
│           ├── stores/
│           │   └── settings.svelte.ts # Svelte 5 Rune-based reactive state store
│           ├── components/
│           │   ├── Sidebar.svelte  # Sidebar: Nav tabs with Lucide icons, live switch
│           │   ├── Switch.svelte   # Toggle switch with smooth slide animation
│           │   ├── Keycap.svelte   # Physical keyboard keycap component ([Ctrl] + [Drag])
│           │   ├── Card.svelte     # Reusable card container with border & background
│           │   └── Segment.svelte  # Segmented pill control (Modifier & Theme selector)
│           └── tabs/
│               ├── TabHome.svelte      # Hero title, active badge, keycap trigger, tips
│               ├── TabPlugins.svelte   # Column-wise plugin cards with install/uninstall
│               └── TabSettings.svelte  # Preferences: modifier, theme, autostart, tray
```

---

## 3. UI Specifications & Styling Rules

### Colors & Theming (Matches Donut HUD & Windows Fluent)
- **Dark Mode (Default)**:
  - Window Background: `#111218`
  - Sidebar Background: `#161822`
  - Card Background: `#1C1E2A`
  - Card Border / Dividers: `#2B2E40`
  - Control Background: `#242738`
  - Text Primary: `#F3F4F6`
  - Text Secondary: `#9CA3AF`
  - Text Muted: `#6B7280`
  - Accent / Primary Blue: `#7066E0` (from Donut slice hover state)
  - Success Green: `#22C55E`
  - Danger / Uninstall Red: `#EF4444` (light red container: `#521E24`)
- **Light Mode**:
  - Window Background: `#F8FAFC`
  - Sidebar Background: `#F1F5F9`
  - Card Background: `#FFFFFF`
  - Card Border: `#E2E8F0`
  - Control Background: `#EDF2F7`
  - Text Primary: `#0F172A`
  - Text Secondary: `#475569`

### Sidebar Component Specs
- **No brand header**: Clean navigation starts directly at top with uniform padding (`p-3`).
- **Navigation Items**:
  - `Home`: Lucide `Home` icon (16x16) + "Home" text (13px, font-medium).
  - `Plugins`: Lucide `Blocks` icon (16x16) + "Plugins" text (13px, font-medium) + Uninstalled badge counter pill on the right.
  - `Settings`: Lucide `Sliders` icon (16x16) + "Settings" text (13px, font-medium).
- **Alignment Guarantee**: 
  - Using Tailwind CSS flexbox: `flex items-center gap-2.5 px-3 py-2 rounded-lg`.
  - Icon and text share the exact same CSS flexbox alignment axis. 100% pixel-perfect across all display scalings (100%, 125%, 150%).
- **Active Pill State**: `bg-[#7066E0]/15 text-[#7066E0] font-medium` with smooth 150ms transition. No harsh 1px borders.
- **Bottom Service Card**:
  - Row with pulsing green/amber dot, "Active" / "Paused" status text, and slide toggle switch.

### Home Tab Component Specs
- **Header**: "FileUC" title (24px, bold) + "Universal instant file converter and local share engine" subtitle (13px).
- **Banner Row**:
  - Live status pill: green dot + "Engine Active" or amber dot + "Service Paused" in a soft rounded pill.
  - Trigger modifier badge: Keycap `[ Ctrl ]` + `+` + Keycap `[ Drag ]`. Real CSS keycaps with inset shadow and subtle border.
- **Workflow Step Cards**:
  1. "Select Files": "Pick any file, image, document, video, or audio in Windows Explorer."
  2. "Hold & Drag": "Hold [Modifier] while right-click dragging to summon the Donut HUD ring."
  3. "Drop to Convert": "Release over any action slice to convert or share in milliseconds."
- **Workflow Tips Card**: Clean tips with subtle accent bullet indicators.

### Plugins Tab Component Specs
- Exactly adheres to the requested column-wise specification:
  1. **Title**: Left-aligned (15px, font-semibold).
  2. **Subtitle**: Left-aligned directly below title (12px, text-secondary).
  3. **Bottom Row**:
     - **Left End**: Install status text ("Installed" in green `#22C55E` / "Not installed" in red `#EF4444`).
     - **Right End**: Action button:
       - **Install Button**: Blue `#7066E0` with hover state `#8076EE`.
       - **Uninstall Button**: Soft light red `#521E24` background with `#FF8080` text.
  4. **Progress Bar**: During download, displays a sleek progress bar and percentage (`45% - Downloading...`).

### Settings Tab Component Specs
- Trigger modifier segmented control (`Ctrl` | `Alt` | `Shift`).
- Theme selector (`Dark` | `Light` | `System`).
- Auto-start toggle switch.
- System tray icon toggle switch.
- Save and Cancel action buttons.

---

## 4. Step-by-Step Implementation Plan

### Phase 1: Tauri v2 & Frontend Scaffolding
- [ ] **Step 1.1: Initialize Svelte 5 + Tailwind in `crates/fileuc-settings/ui`**
  - Create `package.json` with `svelte@^5.0.0`, `tailwindcss@^3.4.0`, `lucide-svelte@^0.460.0`, `vite@^5.4.0`.
  - Configure `vite.config.ts` to output to `dist/`.
  - Create `tailwind.config.js` with the FileUC color palette tokens.
  - Create `src/app.css` with dark/light mode variables.

- [ ] **Step 1.2: Configure Tauri v2 in `crates/fileuc-settings`**
  - Add `tauri = { version = "2", features = [] }` to `crates/fileuc-settings/Cargo.toml`.
  - Add `tauri-build = "2"` to `[build-dependencies]`.
  - Create `build.rs` calling `tauri_build::build()`.
  - Create `tauri.conf.json`:
    - Window size: `800 x 560`, fixed size (`resizable: false`), centered.
    - Title: `"FileUC Preferences"`.
    - `frontendDist`: `"../ui/dist"`.
    - Permissions: Core window and invoke permissions.

### Phase 2: Svelte 5 Components Implementation
- [ ] **Step 2.1: Implement Reusable Core UI Components**
  - `src/components/Switch.svelte`: Smooth slide toggle switch.
  - `src/components/Keycap.svelte`: Physical keyboard keycap badge.
  - `src/components/Segment.svelte`: Segmented control pill selector.
  - `src/components/Card.svelte`: Themed card container.
  - `src/components/Button.svelte`: Primary, secondary, and danger button variants.

- [ ] **Step 2.2: Implement Navigation Sidebar (`src/components/Sidebar.svelte`)**
  - Top tabs: Home (Home icon), Plugins (Blocks icon), Settings (Sliders icon).
  - Uninstalled badge pill counter on Plugins tab.
  - Pixel-perfect flexbox alignment: `flex items-center gap-3`.
  - Bottom service active/paused status card with toggle switch.

- [ ] **Step 2.3: Implement Tab Views**
  - `src/tabs/TabHome.svelte`: Header, status pill, physical trigger keycap, 3 step cards, tips.
  - `src/tabs/TabPlugins.svelte`: Column-wise cards (Title, Subtitle, Green/Red status on left, Blue Install / Light Red Uninstall button on right, download progress bar).
  - `src/tabs/TabSettings.svelte`: Modifiers, Appearance, Startup & Tray toggles, Save/Cancel buttons.

- [ ] **Step 2.4: Implement Reactive Store & Tauri IPC Bridge (`src/stores/settings.svelte.ts`)**
  - Svelte 5 `$state` runes for reactive properties.
  - `invoke("get_settings")`, `invoke("save_settings")`, `invoke("toggle_service")`, `invoke("install_plugin")`, etc.
  - Real-time progress events via Tauri event listener (`listen("plugin-progress")`).

### Phase 3: Rust Backend Integration
- [ ] **Step 3.1: Implement Tauri Commands in `crates/fileuc-settings/src/commands.rs`**
  - `get_settings`: Reads `AppSettings` from `SettingsStore`.
  - `save_settings`: Persists `AppSettings`, syncs autostart, and sends IPC notification to daemon.
  - `toggle_service`: Toggles service state and notifies daemon.
  - `get_plugin_status`: Probes FFmpeg and docx layout engines.
  - `install_plugin` / `uninstall_plugin`: Dispatches async download with event progress.

- [ ] **Step 3.2: Implement `run_tauri()` and `SettingsController` in `crates/fileuc-settings/src/lib.rs`**
  - Export `run_tauri()` entry point.
  - Update `SettingsController`:
    - `show()` / `show_tab()`: Spawns `fileuc.exe --settings` (or opens window via Tauri).
    - If settings window is already open, brings it to foreground.
    - When user closes window, process cleanly terminates.

- [ ] **Step 3.3: Wire CLI Flag in `crates/fileuc-daemon/src/main.rs`**
  - Check `std::env::args()` for `--settings` or `settings`.
  - If present, immediately execute `fileuc_settings::run_tauri()` and return.

---

## 5. Verification & Testing

### Verification Steps
1. **Frontend Build Verification**:
   - Run `npm run build` inside `crates/fileuc-settings/ui`.
   - Verify static assets generate in `dist/` with zero errors.
2. **Settings Window Standalone Run**:
   - Run `cargo run -p fileuc-settings --bin fileuc-settings` (or `cargo run -p fileuc-daemon -- --settings`).
   - Verify window opens with exact 800x560 dimensions.
   - Inspect sidebar: verify Home, Plugins, Settings icons and labels are 100% aligned on the exact horizontal center line.
   - Verify Dark and Light mode theme switching works instantly.
   - Verify Plugin install/uninstall status colors (green for installed, red for not installed).
   - Verify Install button is Donut blue (`#7066E0`), Uninstall button is light red (`#521E24`).
3. **Full Daemon Integration**:
   - Run `cargo build --release --bin fileuc`.
   - Launch `fileuc.exe`. Verify background daemon idles at < 8 MB RAM.
   - Click "Settings" in system tray: verify settings window opens smoothly.
   - Close settings window: verify WebView2 processes terminate and memory drops back to < 8 MB.
   - Verify Donut HUD ring (`Ctrl + Right Drag`) operates at 120/60 FPS with zero interference.
