use std::path::{Path, PathBuf};

/// Resolves an output file path with the specified extension in the same directory as `src`.
/// If the destination file already exists or would overwrite `src`, appends ` (2)`, ` (3)`, etc.
pub fn resolve_output_path(src: &Path, ext: &str) -> PathBuf {
    let parent = src.parent().unwrap_or_else(|| Path::new(""));
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let clean_ext = ext.trim_start_matches('.');

    let initial = parent.join(format!("{}.{}", stem, clean_ext));
    if !initial.exists() && initial != src {
        return initial;
    }

    let mut counter = 2;
    loop {
        let candidate = parent.join(format!("{} ({}).{}", stem, counter, clean_ext));
        if !candidate.exists() && candidate != src {
            return candidate;
        }
        counter += 1;
    }
}

/// Resolves an output path with a custom stem suffix (e.g. `_locked` or `_unlocked`).
/// If `strip_existing_suffix` is given (e.g. `_locked` when unlocking), removes it first.
pub fn resolve_suffixed_output_path(
    src: &Path,
    suffix: &str,
    ext: &str,
    strip_existing_suffix: Option<&str>,
) -> PathBuf {
    let parent = src.parent().unwrap_or_else(|| Path::new(""));
    let mut stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    if let Some(strip) = strip_existing_suffix {
        if stem.ends_with(strip) {
            stem.truncate(stem.len() - strip.len());
        }
    }

    let clean_ext = ext.trim_start_matches('.');
    let base_stem = format!("{}{}", stem, suffix);

    let initial = parent.join(format!("{}.{}", base_stem, clean_ext));
    if !initial.exists() && initial != src {
        return initial;
    }

    let mut counter = 2;
    loop {
        let candidate = parent.join(format!("{} ({}).{}", base_stem, counter, clean_ext));
        if !candidate.exists() && candidate != src {
            return candidate;
        }
        counter += 1;
    }
}

/// Cleans a Windows path by ensuring it is absolute and removing any `\\?\` UNC prefix
/// that causes COM errors with Microsoft Office applications.
pub fn clean_windows_path(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let s = abs.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}
