use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn get_cloudflared_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("FileUC").join("bin")
    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
        PathBuf::from(userprofile)
            .join("AppData")
            .join("Local")
            .join("FileUC")
            .join("bin")
    } else {
        PathBuf::from(".").join("bin")
    }
}

pub fn get_cloudflared_path() -> PathBuf {
    get_cloudflared_dir().join("cloudflared.exe")
}

pub fn is_cloudflared_installed() -> bool {
    let path = get_cloudflared_path();
    if let Ok(meta) = std::fs::metadata(&path) {
        meta.is_file() && meta.len() > 10_000_000 // Official binary is ~50MB
    } else {
        false
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadProgress {
    Starting,
    Progress(u8),
    Completed,
    Error(String),
}

pub fn start_download(tx: Sender<DownloadProgress>) -> thread::JoinHandle<Result<(), String>> {
    thread::spawn(move || {
        let dir = get_cloudflared_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            let msg = format!("Failed to create bin dir: {}", e);
            let _ = tx.send(DownloadProgress::Error(msg.clone()));
            return Err(msg);
        }

        let target_path = get_cloudflared_path();
        if is_cloudflared_installed() {
            let _ = tx.send(DownloadProgress::Completed);
            return Ok(());
        }

        let tmp_path = dir.join("cloudflared.tmp");
        let _ = tx.send(DownloadProgress::Starting);

        let url = "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe";

        let mut cmd = Command::new("curl.exe");
        cmd.arg("-L")
            .arg("-f")
            .arg("-s")
            .arg("-S")
            .arg("-o")
            .arg(&tmp_path)
            .arg(url);

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let status = match cmd.status() {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("Failed to invoke curl: {}", e);
                let _ = tx.send(DownloadProgress::Error(msg.clone()));
                return Err(msg);
            }
        };

        if !status.success() {
            let _ = std::fs::remove_file(&tmp_path);
            let msg = format!("Download failed with exit code: {:?}", status.code());
            let _ = tx.send(DownloadProgress::Error(msg.clone()));
            return Err(msg);
        }

        // Verify downloaded file size
        if let Ok(meta) = std::fs::metadata(&tmp_path) {
            if meta.len() < 10_000_000 {
                let _ = std::fs::remove_file(&tmp_path);
                let msg = "Downloaded file is corrupted or too small".to_string();
                let _ = tx.send(DownloadProgress::Error(msg.clone()));
                return Err(msg);
            }
        } else {
            let msg = "Downloaded file not found".to_string();
            let _ = tx.send(DownloadProgress::Error(msg.clone()));
            return Err(msg);
        }

        // Rename tmp to target
        if let Err(e) = std::fs::rename(&tmp_path, &target_path) {
            // Try remove and rename if target exists
            let _ = std::fs::remove_file(&target_path);
            if let Err(e2) = std::fs::rename(&tmp_path, &target_path) {
                let msg = format!("Failed to save cloudflared: {} ({})", e, e2);
                let _ = tx.send(DownloadProgress::Error(msg.clone()));
                return Err(msg);
            }
        }

        let _ = tx.send(DownloadProgress::Completed);
        Ok(())
    })
}

pub struct CloudflareTunnel {
    child: Arc<Mutex<Option<Child>>>,
    pub rx_url: Receiver<Result<String, String>>,
    stopped: Arc<AtomicBool>,
}

impl CloudflareTunnel {
    pub fn start(local_port: u16, path_suffix: &str) -> Result<Self, String> {
        let (tx, rx) = channel();
        let path_suffix = path_suffix.to_string();
        let exe_path = get_cloudflared_path();

        if !is_cloudflared_installed() {
            return Err("cloudflared is not installed".to_string());
        }

        let child_holder = Arc::new(Mutex::new(None));
        let child_ref = child_holder.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = stopped.clone();

        thread::spawn(move || {
            if stopped_clone.load(Ordering::SeqCst) {
                return;
            }

            let mut cmd = Command::new(&exe_path);
            cmd.arg("tunnel")
                .arg("--url")
                .arg(format!("http://127.0.0.1:{}", local_port))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            #[cfg(windows)]
            {
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            match cmd.spawn() {
                Ok(mut child) => {
                    if stopped_clone.load(Ordering::SeqCst) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                    let stderr = child.stderr.take();
                    let stdout = child.stdout.take();
                    {
                        let mut lock = child_ref.lock().unwrap_or_else(|e| e.into_inner());
                        if stopped_clone.load(Ordering::SeqCst) {
                            let _ = child.kill();
                            let _ = child.wait();
                            return;
                        }
                        *lock = Some(child);
                    }

                    let mut found_url = None;

                    // cloudflared logs tunnel info to stderr
                    if let Some(err) = stderr {
                        let reader = BufReader::new(err);
                        for line in reader.lines() {
                            if stopped_clone.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Ok(l) = line {
                                if let Some(url) = extract_trycloudflare_url(&l) {
                                    found_url = Some(url);
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    // Fallback to check stdout if not in stderr
                    if found_url.is_none() && !stopped_clone.load(Ordering::SeqCst) {
                        if let Some(out) = stdout {
                            let reader = BufReader::new(out);
                            for line in reader.lines() {
                                if stopped_clone.load(Ordering::SeqCst) {
                                    break;
                                }
                                if let Ok(l) = line {
                                    if let Some(url) = extract_trycloudflare_url(&l) {
                                        found_url = Some(url);
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                        }
                    }

                    if stopped_clone.load(Ordering::SeqCst) {
                        return;
                    }

                    if let Some(base_url) = found_url {
                        let full_url = if path_suffix.starts_with('/') {
                            format!("{}{}", base_url, path_suffix)
                        } else {
                            format!("{}/{}", base_url, path_suffix)
                        };
                        let _ = tx.send(Ok(full_url));
                    } else {
                        let _ = tx.send(Err("Failed to extract Cloudflare tunnel URL from output".to_string()));
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to spawn cloudflared: {}", e)));
                }
            }
        });

        Ok(Self {
            child: child_holder,
            rx_url: rx,
            stopped,
        })
    }

    pub fn stop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut lock) = self.child.lock() {
            if let Some(mut child) = lock.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

impl Drop for CloudflareTunnel {
    fn drop(&mut self) {
        self.stop();
    }
}

fn extract_trycloudflare_url(line: &str) -> Option<String> {
    if let Some(idx) = line.find("https://") {
        let remainder = &line[idx..];
        let end_idx = remainder
            .find(|c: char| c.is_whitespace() || c == '|' || c == '"' || c == '\'' || c == '>')
            .unwrap_or(remainder.len());
        let candidate = &remainder[..end_idx];
        if candidate.contains(".trycloudflare.com") {
            return Some(candidate.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_trycloudflare_url() {
        let log1 = "2026-11-15T12:00:00Z INF |  https://purple-sunset-zone.trycloudflare.com  |";
        assert_eq!(
            extract_trycloudflare_url(log1),
            Some("https://purple-sunset-zone.trycloudflare.com".to_string())
        );

        let log2 = "Your quick Tunnel has been created! Visit it at: https://my-subdomain.trycloudflare.com";
        assert_eq!(
            extract_trycloudflare_url(log2),
            Some("https://my-subdomain.trycloudflare.com".to_string())
        );

        let log3 = "Connecting to edge...";
        assert_eq!(extract_trycloudflare_url(log3), None);
    }
}
