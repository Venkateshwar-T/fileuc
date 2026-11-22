use fileuc_types::ConvertError;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
const BUNDLED_7Z_EXE: &[u8] = include_bytes!("../assets/engines/7zip/7z.exe");
#[cfg(windows)]
const BUNDLED_7Z_DLL: &[u8] = include_bytes!("../assets/engines/7zip/7z.dll");

/// Extracts the embedded standalone 7-Zip engine to %LOCALAPPDATA%\FileUC\engines\7zip\
/// on demand. Ensures FileUC is 100% self-contained with zero external dependencies.
#[cfg(windows)]
pub fn ensure_bundled_7z() -> Option<PathBuf> {
    static EXTRACTED: OnceLock<Option<PathBuf>> = OnceLock::new();
    EXTRACTED
        .get_or_init(|| {
            let base_dir = std::env::var_os("LOCALAPPDATA")
                .or_else(|| std::env::var_os("APPDATA"))
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);

            let engine_dir = base_dir.join("FileUC").join("engines").join("7zip");
            if std::fs::create_dir_all(&engine_dir).is_err() {
                return None;
            }

            let exe_path = engine_dir.join("7z.exe");
            let dll_path = engine_dir.join("7z.dll");

            let need_write_exe = match std::fs::metadata(&exe_path) {
                Ok(m) => m.len() != BUNDLED_7Z_EXE.len() as u64,
                Err(_) => true,
            };
            if need_write_exe {
                if std::fs::write(&exe_path, BUNDLED_7Z_EXE).is_err() {
                    return None;
                }
            }

            let need_write_dll = match std::fs::metadata(&dll_path) {
                Ok(m) => m.len() != BUNDLED_7Z_DLL.len() as u64,
                Err(_) => true,
            };
            if need_write_dll {
                if std::fs::write(&dll_path, BUNDLED_7Z_DLL).is_err() {
                    return None;
                }
            }

            if exe_path.is_file() {
                Some(exe_path)
            } else {
                None
            }
        })
        .clone()
}

#[cfg(not(windows))]
pub fn ensure_bundled_7z() -> Option<PathBuf> {
    None
}

/// Finds an available 7-Zip (`7z.exe` or `7za.exe`) binary.
/// Prioritizes the embedded standalone engine so FileUC works with zero external software installed.
pub fn find_7z() -> Option<PathBuf> {
    static CACHED: OnceLock<Option<PathBuf>> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            // 1. Next to current executable (e.g. portable deployment)
            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    let next_to_exe = parent.join("7z.exe");
                    if next_to_exe.is_file() {
                        return Some(next_to_exe);
                    }
                    let engines_7z = parent.join("engines").join("7zip").join("7z.exe");
                    if engines_7z.is_file() {
                        return Some(engines_7z);
                    }
                }
            }

            // 2. Embedded bundled standalone engine (guarantees 100% self-contained zero-dependency execution)
            if let Some(bundled) = ensure_bundled_7z() {
                return Some(bundled);
            }

            // 3. Standard Program Files paths on Windows
            let candidates = [
                PathBuf::from(r"C:\Program Files\7-Zip\7z.exe"),
                PathBuf::from(r"C:\Program Files (x86)\7-Zip\7z.exe"),
                PathBuf::from(r"C:\Program Files\7-Zip\7za.exe"),
                PathBuf::from(r"C:\Program Files (x86)\7-Zip\7za.exe"),
            ];
            for c in &candidates {
                if c.is_file() {
                    return Some(c.clone());
                }
            }

            // 4. Fall back to system PATH
            let mut cmd = std::process::Command::new("7z");
            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    return Some(PathBuf::from("7z"));
                }
            }
            None
        })
        .clone()
}

/// Helper to parse real-time progress percentages from 7z CLI stdout with `-bsp1`.
/// Returns `false` if cancelled cooperatively, `true` if completed.
fn stream_7z_progress<R: Read>(
    reader: R,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    on_progress: &dyn Fn(f32),
) -> bool {
    let mut reader = std::io::BufReader::with_capacity(4096, reader);
    let mut num_buf = Vec::with_capacity(8);
    let mut byte = [0u8; 1];
    let mut last_pct = 0;
    while let Ok(1) = reader.read(&mut byte) {
        if let Some(c) = cancel {
            if c.load(Ordering::Relaxed) {
                return false;
            }
        }
        let b = byte[0];
        if b == b'%' {
            if let Ok(s) = std::str::from_utf8(&num_buf) {
                if let Ok(pct) = s.trim().parse::<u32>() {
                    if pct <= 100 && pct > last_pct {
                        last_pct = pct;
                        on_progress(pct as f32 / 100.0);
                    }
                }
            }
            num_buf.clear();
        } else if b.is_ascii_digit() {
            num_buf.push(b);
            if num_buf.len() > 3 {
                num_buf.remove(0);
            }
        } else {
            num_buf.clear();
        }
    }
    if last_pct < 100 {
        on_progress(1.0);
    }
    true
}

/// Recursively calculates the total uncompressed bytes across all input sources.
pub fn calculate_total_bytes(sources: &[PathBuf]) -> u64 {
    let mut total = 0u64;
    for s in sources {
        if s.is_file() {
            if let Ok(m) = s.metadata() {
                total += m.len();
            }
        } else if s.is_dir() {
            total += dir_total_bytes(s);
        }
    }
    total
}

fn dir_total_bytes(dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Ok(m) = p.metadata() {
                    total += m.len();
                }
            } else if p.is_dir() {
                total += dir_total_bytes(&p);
            }
        }
    }
    total
}

/// Read wrapper that emits cumulative progress as bytes are read, checking for cancellation.
struct ProgressReader<'a, R> {
    inner: R,
    bytes: Arc<AtomicU64>,
    total: u64,
    cancel: Option<&'a std::sync::atomic::AtomicBool>,
    callback: &'a dyn Fn(f32),
    last_reported: AtomicU32,
}

impl<'a, R> ProgressReader<'a, R> {
    fn new(
        inner: R,
        bytes: Arc<AtomicU64>,
        total: u64,
        cancel: Option<&'a std::sync::atomic::AtomicBool>,
        callback: &'a dyn Fn(f32),
    ) -> Self {
        Self {
            inner,
            bytes,
            total,
            cancel,
            callback,
            last_reported: AtomicU32::new(0),
        }
    }
}

impl<'a, R: Read> Read for ProgressReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(c) = self.cancel {
            if c.load(Ordering::Relaxed) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "Cancelled by user",
                ));
            }
        }
        let n = self.inner.read(buf)?;
        if n > 0 && self.total > 0 {
            let current = self.bytes.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
            let pct = ((current as f64 / self.total as f64) * 100.0) as u32;
            let last = self.last_reported.load(Ordering::Relaxed);
            if pct > last && pct <= 99 {
                if self.last_reported.compare_exchange(last, pct, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
                    (self.callback)(pct as f32 / 100.0);
                }
            }
        }
        Ok(n)
    }
}

fn find_common_parent(paths: &[PathBuf]) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    let first = &paths[0];
    let parent = first.parent()?;
    let mut common = parent.to_path_buf();
    for p in &paths[1..] {
        while !p.starts_with(&common) {
            if !common.pop() {
                return None;
            }
        }
    }
    if common.as_os_str().is_empty() {
        None
    } else {
        Some(common)
    }
}

fn run_7z_archive_command(
    format_arg: Option<&str>,
    sources: &[PathBuf],
    output: &Path,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    on_progress: impl Fn(f32),
) -> Result<(), ConvertError> {
    if sources.is_empty() {
        return Err(ConvertError::ConverterError("No sources provided to compress".to_string()));
    }

    if let Some(ref c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(ConvertError::ConverterError("Cancelled by user".to_string()));
        }
    }

    for src in sources {
        if !src.exists() {
            return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
        }
    }

    // Ensure pre-existing output is cleared so 7z creates a clean archive rather than appending
    if output.exists() {
        let _ = std::fs::remove_file(output);
    }
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let z_exe = find_7z().ok_or_else(|| {
        ConvertError::ConverterError("7-Zip standalone engine is unavailable".to_string())
    })?;

    let abs_output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir().map(|cwd| cwd.join(output)).unwrap_or_else(|_| output.to_path_buf())
    };

    let common_parent = find_common_parent(sources);

    let mut cmd = std::process::Command::new(&z_exe);
    cmd.arg("a");
    if let Some(fmt) = format_arg {
        cmd.arg(fmt);
    }
    // High-throughput fast compression (-mx=1) with multi-threading:
    // Compresses 12GB+ files at disk I/O throughput (hundreds of MB/s) rather than CPU bottleneck
    cmd.arg("-mx=1");
    cmd.arg("-y")
        .arg("-bsp1")
        .arg("-mmt=on")
        .arg("--")
        .arg(&abs_output);

    if let Some(ref cp) = common_parent {
        cmd.current_dir(cp);
        for src in sources {
            if let Ok(rel) = src.strip_prefix(cp) {
                cmd.arg(rel);
            } else {
                cmd.arg(src);
            }
        }
    } else {
        for src in sources {
            cmd.arg(src);
        }
    }

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        ConvertError::ConverterError(format!("Failed to execute 7-Zip engine: {}", e))
    })?;

    let mut was_cancelled = false;
    if let Some(stdout) = child.stdout.take() {
        let cancel_ref = cancel.as_deref();
        if !stream_7z_progress(stdout, cancel_ref, &on_progress) {
            was_cancelled = true;
            let _ = child.kill();
        }
    }

    let status = child.wait().map_err(|e| {
        ConvertError::ConverterError(format!("Failed waiting for 7-Zip engine: {}", e))
    })?;

    if was_cancelled {
        if output.exists() {
            let _ = std::fs::remove_file(output);
        }
        return Err(ConvertError::ConverterError("Cancelled by user".to_string()));
    }

    if status.success() {
        on_progress(1.0);
        Ok(())
    } else {
        let mut err_msg = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut err_msg);
        }
        Err(ConvertError::ConverterError(format!(
            "Archive compression failed (exit code {:?}): {}",
            status.code(),
            err_msg.trim()
        )))
    }
}

pub fn create_zip_with_progress_and_cancel(
    sources: &[PathBuf],
    output: &Path,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    on_progress: impl Fn(f32),
) -> Result<(), ConvertError> {
    if find_7z().is_some() {
        return run_7z_archive_command(Some("-tzip"), sources, output, cancel, on_progress);
    }

    create_zip_pure_rust(sources, output, cancel, on_progress)
}

pub fn create_zip_with_progress(
    sources: &[PathBuf],
    output: &Path,
    on_progress: impl Fn(f32),
) -> Result<(), ConvertError> {
    create_zip_with_progress_and_cancel(sources, output, None, on_progress)
}

fn create_zip_pure_rust(
    sources: &[PathBuf],
    output: &Path,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    on_progress: impl Fn(f32),
) -> Result<(), ConvertError> {
    if sources.is_empty() {
        return Err(ConvertError::ConverterError("No sources provided to zip".to_string()));
    }

    if let Some(ref c) = cancel {
        if c.load(Ordering::Relaxed) {
            return Err(ConvertError::ConverterError("Cancelled by user".to_string()));
        }
    }

    for src in sources {
        if !src.exists() {
            return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
        }
    }

    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let total_bytes = calculate_total_bytes(sources);
    let bytes_read = Arc::new(AtomicU64::new(0));

    let file = File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let buffered = std::io::BufWriter::with_capacity(256 * 1024, file);
    let mut zip = ZipWriter::new(buffered);
    // Explicitly enable Zip64 format for large multi-gigabyte (>= 4GB) archives:
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);

    let cancel_ref = cancel.as_deref();

    for src in sources {
        if src.is_file() {
            let file_name = src
                .file_name()
                .ok_or_else(|| ConvertError::ConverterError("Invalid file name".to_string()))?
                .to_string_lossy();

            zip.start_file(file_name, options)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

            let f = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
            let mut tracked = ProgressReader::new(f, bytes_read.clone(), total_bytes, cancel_ref, &on_progress);
            if let Err(e) = std::io::copy(&mut tracked, &mut zip) {
                if output.exists() {
                    let _ = std::fs::remove_file(output);
                }
                return Err(ConvertError::IoError(e.to_string()));
            }
        } else if src.is_dir() {
            if let Err(e) = add_dir_to_zip_tracked(&mut zip, src, src, options, bytes_read.clone(), total_bytes, cancel_ref, &on_progress) {
                if output.exists() {
                    let _ = std::fs::remove_file(output);
                }
                return Err(e);
            }
        }
    }

    zip.finish().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    on_progress(1.0);
    Ok(())
}

pub fn create_zip(sources: &[PathBuf], output: &Path) -> Result<(), ConvertError> {
    create_zip_with_progress(sources, output, |_| {})
}

fn add_dir_to_zip_tracked<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    base_dir: &Path,
    current_dir: &Path,
    options: SimpleFileOptions,
    bytes_read: Arc<AtomicU64>,
    total_bytes: u64,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress_cb: &dyn Fn(f32),
) -> Result<(), ConvertError> {
    let entries = std::fs::read_dir(current_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    for entry in entries.flatten() {
        if entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(base_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if path.is_file() {
            zip.start_file(relative, options)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            let f = File::open(&path).map_err(|e| ConvertError::IoError(e.to_string()))?;
            let mut tracked = ProgressReader::new(f, bytes_read.clone(), total_bytes, cancel, progress_cb);
            std::io::copy(&mut tracked, zip).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if path.is_dir() {
            zip.add_directory(format!("{}/", relative), options)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            add_dir_to_zip_tracked(zip, base_dir, &path, options, bytes_read.clone(), total_bytes, cancel, progress_cb)?;
        }
    }
    Ok(())
}

pub fn extract_archive(src: &Path, output_dir: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    std::fs::create_dir_all(output_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    let file_name = src.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();

    // 1. Fast multi-threaded native extraction via 7-Zip (handles .7z, .rar, .tar, .tar.gz, .tgz, .gz, .bz2, .xz, .zip)
    if let Some(z_exe) = find_7z() {
        let mut cmd = std::process::Command::new(&z_exe);
        cmd.arg("x")
            .arg("-y")
            .arg("-p-")
            .arg(format!("-o{}", output_dir.to_string_lossy()))
            .arg("--")
            .arg(src);

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                return Ok(());
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                if !err.trim().is_empty() {
                    return Err(ConvertError::ConverterError(format!("Extraction failed: {}", err.trim())));
                }
            }
        }
    }

    // 2. Pure-Rust fallbacks for tar and zip if engine is unavailable
    if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        let tar_gz = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive
            .unpack(output_dir)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to unpack tar.gz: {}", e)))?;
        return Ok(());
    }

    if file_name.ends_with(".tar") {
        let tar_file = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
        let mut archive = tar::Archive::new(tar_file);
        archive
            .unpack(output_dir)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to unpack tar: {}", e)))?;
        return Ok(());
    }

    // Default to ZIP
    let file = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open zip archive: {}", e)))?;

    const MAX_FILES: usize = 50_000;
    const MAX_TOTAL_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GB decompression quota

    if archive.len() > MAX_FILES {
        return Err(ConvertError::ConverterError(format!(
            "Archive contains too many files ({} > limit of {})",
            archive.len(),
            MAX_FILES
        )));
    }

    let mut total_decompressed_bytes: u64 = 0;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to read zip entry: {}", e)))?;

        let enclosed = match entry.enclosed_name() {
            Some(path) => path.to_owned(),
            None => {
                return Err(ConvertError::ConverterError(format!(
                    "Potentially dangerous archive entry path escapes target directory: {}",
                    entry.name()
                )));
            }
        };

        let out_path = output_dir.join(&enclosed);

        if !out_path.starts_with(output_dir) {
            return Err(ConvertError::ConverterError(format!(
                "Archive entry escapes destination: {}",
                entry.name()
            )));
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| ConvertError::IoError(e.to_string()))?;
                }
            }
            let mut outfile = File::create(&out_path).map_err(|e| ConvertError::IoError(e.to_string()))?;
            let remaining_quota = MAX_TOTAL_BYTES.saturating_sub(total_decompressed_bytes);
            let mut limited_reader = entry.by_ref().take(remaining_quota + 1);
            let written = std::io::copy(&mut limited_reader, &mut outfile)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            total_decompressed_bytes = total_decompressed_bytes.saturating_add(written);
            if total_decompressed_bytes > MAX_TOTAL_BYTES {
                drop(outfile);
                let _ = std::fs::remove_file(&out_path);
                return Err(ConvertError::ConverterError(
                    "Archive decompression quota exceeded (potential decompression bomb)".to_string(),
                ));
            }
        }
    }

    Ok(())
}

pub fn create_7z_with_progress_and_cancel(
    sources: &[PathBuf],
    output: &Path,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    on_progress: impl Fn(f32),
) -> Result<(), ConvertError> {
    run_7z_archive_command(None, sources, output, cancel, on_progress)
}

pub fn create_7z_with_progress(
    sources: &[PathBuf],
    output: &Path,
    on_progress: impl Fn(f32),
) -> Result<(), ConvertError> {
    create_7z_with_progress_and_cancel(sources, output, None, on_progress)
}

pub fn create_7z(sources: &[PathBuf], output: &Path) -> Result<(), ConvertError> {
    create_7z_with_progress(sources, output, |_| {})
}
