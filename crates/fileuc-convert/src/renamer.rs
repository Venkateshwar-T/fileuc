use fileuc_types::ConvertError;
use std::path::{Path, PathBuf};

/// Renames a batch of files and/or folders using `base_name` followed by auto-incrementing
/// numbered suffixes (e.g. `Trip_01.jpg`, `Trip_02.jpg`), strictly respecting the order
/// of the input slice (selection order).
///
/// Uses a two-phase rename (via temporary intermediate names) to prevent collisions
/// if items are already named similarly in the target directory.
pub fn bulk_rename(items: &[PathBuf], base_name: &str) -> Result<Vec<PathBuf>, ConvertError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let clean_base = base_name.trim();
    if clean_base.is_empty() {
        return Err(ConvertError::ConverterError("Base name cannot be empty".to_string()));
    }

    // Determine zero-padding width (minimum 2 digits if >= 10 items, matching e.g. Trip_01.jpg)
    let count = items.len();
    let pad_width = if count >= 100 {
        3
    } else if count >= 10 {
        2
    } else {
        2 // Default to 2-digit padding as in Trip_01 for clean explorer sorting
    };

    let mut planned_targets = Vec::with_capacity(count);

    for (idx, path) in items.iter().enumerate() {
        if !path.exists() {
            return Err(ConvertError::SourceNotFound(path.to_string_lossy().to_string()));
        }

        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let ext = if path.is_file() {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default()
        } else {
            String::new()
        };

        let new_file_name = format!("{}_{:0width$}{}", clean_base, idx + 1, ext, width = pad_width);
        let dest_path = parent.join(new_file_name);
        planned_targets.push(dest_path);
    }

    // Phase 1: Rename to temporary names in the same directory to avoid in-place collisions
    let mut temp_paths = Vec::with_capacity(count);
    let session_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(123456);

    for (idx, path) in items.iter().enumerate() {
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let temp_name = format!(".fileuc_tmp_rename_{}_{}", session_id, idx);
        let temp_path = parent.join(temp_name);

        if let Err(e) = std::fs::rename(path, &temp_path) {
            // Roll back already renamed items in Phase 1
            for (rollback_idx, temp_renamed) in temp_paths.iter().enumerate() {
                let _ = std::fs::rename(temp_renamed, &items[rollback_idx]);
            }
            return Err(ConvertError::IoError(format!(
                "Failed temporary rename for '{}': {}. Changes were rolled back.",
                path.display(),
                e
            )));
        }
        temp_paths.push(temp_path);
    }

    // Phase 2: Move from temporary names to final target names
    let mut final_paths = Vec::with_capacity(count);
    for (idx, (temp_path, dest_path)) in temp_paths.iter().zip(planned_targets.iter()).enumerate() {
        if let Err(e) = std::fs::rename(temp_path, dest_path) {
            // Roll back Phase 2 completed items (from destination back to original)
            for (i, final_renamed) in final_paths.iter().enumerate() {
                let _ = std::fs::rename(final_renamed, &items[i]);
            }
            // Roll back remaining Phase 1 temp items (from temp back to original)
            for i in idx..temp_paths.len() {
                let _ = std::fs::rename(&temp_paths[i], &items[i]);
            }
            return Err(ConvertError::IoError(format!(
                "Failed final rename to '{}': {}. All renames rolled back.",
                dest_path.display(),
                e
            )));
        }
        final_paths.push(dest_path.clone());
    }

    Ok(final_paths)
}
