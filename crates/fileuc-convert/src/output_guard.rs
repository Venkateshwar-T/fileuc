use std::path::{Path, PathBuf};

/// RAII Guard that removes an uncommitted output file if a conversion operation
/// fails with an error or panics before completion.
///
/// Once the operation succeeds, calling `.disarm()` commits the file and prevents deletion.
pub struct OutputGuard {
    path: PathBuf,
    armed: bool,
}

impl OutputGuard {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            armed: true,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Disarms the guard, committing the file and returning its PathBuf.
    pub fn disarm(mut self) -> PathBuf {
        self.armed = false;
        std::mem::take(&mut self.path)
    }
}

impl Drop for OutputGuard {
    fn drop(&mut self) {
        if self.armed && self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
