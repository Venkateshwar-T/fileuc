<p align="center">
  <img src="assets/app-icon.png" width="96" height="96" alt="FileUC Icon" />
</p>

<h1 align="center">FileUC</h1>

<p align="center">
  <strong>Instant Desktop File Utility, Quick Actions & Mobile Sharing Engine for Windows</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-0078D6?style=flat-square&logo=windows" alt="Platform" />
  <img src="https://img.shields.io/badge/Rust-1.80%2B-orange?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Frontend-Tauri%20v2%20%2B%20Svelte%205-FF3E00?style=flat-square&logo=svelte" alt="Svelte 5" />
  <img src="https://img.shields.io/badge/Renderer-Tiny--Skia%20(60FPS)-4A90E2?style=flat-square" alt="Renderer" />
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License" />
</p>

---

## ✦ What is FileUC?

**FileUC** is a blazing-fast, privacy-first desktop file utility for Windows that supercharges your daily file workflow. Instead of searching for online converter websites or launching heavyweight applications, FileUC brings an interactive, spring-animated **Radial Donut HUD** directly beneath your cursor.

Hold <kbd>Ctrl</kbd> and drag any file or group of files to summon the HUD, then simply drop on a slice to convert, rename, compress, protect, or share in milliseconds.

---

## ✦ Key Features

### 🍩 Interactive Radial Donut HUD
- **Gesture-Activated**: Hold <kbd>Ctrl</kbd> while dragging files (or configure your own trigger in Settings) to summon the radial menu.
- **Spring Physics & 60 FPS**: Rendered via Tiny-Skia directly onto a per-pixel alpha layered Win32 window with smooth spring animations.
- **Context-Aware Smart Filtering**: The radial slices adapt dynamically based on the exact files you drag (Images, PDFs, Word, Excel, Media, or mixed batches).

### ⚡ Universal File Conversions (100% Local & Private)
- **Images**: Lossless & high-quality conversion between **PNG**, **JPG**, **WebP**, **SVG**, **ICO**, **BMP**, and **TIFF** with color profile and DPI preservation.
- **PDF Suite**:
  - **PDF to Word (DOCX)**: Structural paragraph, font, and layout reconstruction.
  - **Office to PDF**: Instant Word (`.docx`) and Excel (`.xlsx`) to PDF rendering.
  - **Markdown / Text to PDF & HTML**: Built-in syntax parsing and typographic layout.
  - **PDF Tools**: Merge multiple PDFs or split multi-page documents with one drop.
- **Video & Audio** *(via on-demand FFmpeg plugin)*:
  - Video to GIF (with optimized palette generation).
  - Video to MP3 extraction.
  - Video Compression (H.264 / AAC rate control).
- **Archives**: One-click Zip compression and extraction.

### 📱 Instant Mobile & Web Sharing
- **Local Wi-Fi Sharing**: Starts an ephemeral, secure local HTTP server and instantly displays a scan-to-download QR code right on the HUD. Anyone on your local Wi-Fi network can download the files immediately.
- **Global Internet Sharing**: Free, secure worldwide sharing powered by integrated Cloudflare Tunnels (`cloudflared`). Generates a temporary public HTTPS link and QR code accessible from anywhere on the planet without port forwarding.

### 🔒 Document Security & Encryption
- **Lock PDF**: Military-grade AES-256 PDF encryption with an interactive password card right on the HUD.
- **Unlock PDF**: Decrypt password-protected PDFs on the fly.

### 🏷️ Smart Bulk Renaming
- Drop multiple files over **Rename** to open an on-HUD live preview card.
- Type your desired base name with live boundary-checked preview (`trip_01.jpg, trip_02.jpg, ... trip_12.jpg`) and hit <kbd>Enter</kbd> to rename instantaneously.

### ⚙️ Modern Settings & Plugin Manager
- Built with **Tauri 2** and **Svelte 5** + Tailwind CSS.
- Sleek glassmorphic dark-mode dashboard.
- 1-click plugin manager to install or remove optional runtime engines (FFmpeg, LibreOffice, Cloudflare).

---

## ✦ Architecture

FileUC is engineered as a high-performance modular Rust workspace:

```
FileUC/
├── crates/
│   ├── fileuc-daemon/     # Background daemon, system tray, IPC hub, HTTP share server
│   ├── fileuc-hook/       # Native Win32 low-level mouse & keyboard gesture recognizer
│   ├── fileuc-hud/        # 60 FPS Tiny-Skia radial donut & card rendering engine
│   ├── fileuc-convert/    # Local conversion pipeline (PDFium, Resvg, Lopdf, Calamine)
│   ├── fileuc-settings/   # Tauri 2 + Svelte 5 settings application
│   └── fileuc-types/      # Shared domain types, geometry math, and IPC protocol
├── installer/             # 1-Click Fast NSIS installer specification
└── .github/workflows/     # Automated release workflow (MSVC + NSIS CI/CD)
```

- **Memory Efficient**: Idles in the background at only ~20–40 MB of RAM.
- **Zero Input Lag**: Uses native `WH_MOUSE_LL` and `WH_KEYBOARD_LL` Windows hooks.
- **Direct Win32 Interop**: Layered window rendering (`UpdateLayeredWindow`) with BGRA swizzling for artifact-free alpha blending.

---

## ✦ Installation

### Option 1: 1-Click Fast Installer (Recommended)
1. Download `FileUC-Setup.exe` from the latest [GitHub Releases](https://github.com/Venkateshwar-T/fileuc/releases).
2. Double-click the installer:
   - Installs to `%LOCALAPPDATA%\FileUC` without requiring Administrator (UAC) permissions.
   - Automatically creates Desktop and Start Menu shortcuts.
   - Instantly launches FileUC in your System Tray.
3. Clean uninstallation is available anytime via **Windows Settings → Installed Apps**.

### Option 2: Portable Executable
Download the standalone `fileuc.exe` from Releases and run it directly.

---

## ✦ Building from Source

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (1.80+ with `x86_64-pc-windows-msvc` target)
- [Node.js](https://nodejs.org/) (v20+ and `npm`)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++ workload)
- [NSIS](https://nsis.sourceforge.io/) (for building the installer)

### Build Instructions

1. **Clone the repository:**
   ```bash
   git clone https://github.com/Venkateshwar-T/fileuc.git
   cd fileuc
   ```

2. **Build the Svelte 5 frontend:**
   ```bash
   cd crates/fileuc-settings/ui
   npm install
   npm run build
   cd ../../..
   ```

3. **Fetch PDFium binary:**
   Download `pdfium.dll` (64-bit Windows) from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases) and place it in the project root alongside `Cargo.toml`.

4. **Build the release binary:**
   ```bash
   cargo build --release --bin fileuc
   ```

5. **Build the 1-Click Installer (Optional):**
   ```bash
   makensis installer/FileUC.nsi
   ```
   The compiled installer will be saved to `target/FileUC-Setup.exe`.

---

## ✦ Running Tests

Run the full suite of automated unit and integration tests:

```bash
cargo test -p fileuc-hud -p fileuc-hook -p fileuc-types
```

---

## ✦ License

This project is licensed under the [MIT License](LICENSE).
