use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct PinggyTunnel {
    child: Arc<Mutex<Option<Child>>>,
    pub rx_url: Receiver<Result<String, String>>,
    stopped: Arc<AtomicBool>,
}

/// Ensures an SSH key pair exists in ~/.ssh so OpenSSH public key authentication
/// connects instantly without password prompts on headless daemons.
fn ensure_ssh_key() {
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let ssh_dir = Path::new(&userprofile).join(".ssh");
        let ed25519 = ssh_dir.join("id_ed25519");
        let rsa = ssh_dir.join("id_rsa");
        if !ed25519.exists() && !rsa.exists() {
            let _ = std::fs::create_dir_all(&ssh_dir);
            let mut keygen = Command::new("ssh-keygen");
            keygen
                .arg("-t")
                .arg("ed25519")
                .arg("-N")
                .arg("")
                .arg("-f")
                .arg(&ed25519);
            #[cfg(windows)]
            {
                keygen.creation_flags(CREATE_NO_WINDOW);
            }
            if let Ok(mut child) = keygen.spawn() {
                let _ = child.wait();
            }
        }
    }
}

impl PinggyTunnel {
    pub fn start(local_port: u16, path_suffix: &str) -> Result<Self, String> {
        let (tx, rx) = channel();
        let path_suffix = path_suffix.to_string();
        let child_holder = Arc::new(Mutex::new(None));
        let child_ref = child_holder.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = stopped.clone();

        thread::spawn(move || {
            if stopped_clone.load(Ordering::SeqCst) {
                return;
            }

            ensure_ssh_key();

            // 1. Primary: Pinggy free tunnel (Fast HTTPS, instant reverse proxy)
            let pinggy_targets = ["free.pinggy.io", "a.pinggy.io"];
            for target in pinggy_targets {
                if stopped_clone.load(Ordering::SeqCst) {
                    return;
                }

                let mut pinggy_cmd = Command::new("ssh");
                pinggy_cmd
                    .arg("-4")
                    .arg("-p")
                    .arg("443")
                    .arg("-o")
                    .arg("StrictHostKeyChecking=no")
                    .arg("-o")
                    .arg("UserKnownHostsFile=NUL")
                    .arg("-o")
                    .arg("BatchMode=yes")
                    .arg("-o")
                    .arg("ServerAliveInterval=30")
                    .arg("-o")
                    .arg("ConnectTimeout=6")
                    .arg("-R")
                    .arg(format!("0:127.0.0.1:{}", local_port))
                    .arg(target)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                #[cfg(windows)]
                {
                    pinggy_cmd.creation_flags(CREATE_NO_WINDOW);
                }

                if let Ok(mut pinggy_child) = pinggy_cmd.spawn() {
                    if stopped_clone.load(Ordering::SeqCst) {
                        let _ = pinggy_child.kill();
                        let _ = pinggy_child.wait();
                        return;
                    }

                    let stdout = pinggy_child.stdout.take();
                    let stderr = pinggy_child.stderr.take();
                    {
                        let mut lock = child_ref.lock().unwrap_or_else(|e| e.into_inner());
                        if stopped_clone.load(Ordering::SeqCst) {
                            let _ = pinggy_child.kill();
                            let _ = pinggy_child.wait();
                            return;
                        }
                        *lock = Some(pinggy_child);
                    }

                    let mut found_url = None;
                    if let Some(out) = stdout {
                        let reader = BufReader::new(out);
                        for line in reader.lines() {
                            if stopped_clone.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Ok(l) = line {
                                if let Some(url) = extract_tunnel_url(&l) {
                                    found_url = Some(url);
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }

                    if found_url.is_none() && !stopped_clone.load(Ordering::SeqCst) {
                        if let Some(err) = stderr {
                            let reader = BufReader::new(err);
                            for line in reader.lines() {
                                if stopped_clone.load(Ordering::SeqCst) {
                                    break;
                                }
                                if let Ok(l) = line {
                                    if let Some(url) = extract_tunnel_url(&l) {
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
                        return;
                    }

                    if let Some(mut old_child) = child_ref.lock().unwrap_or_else(|e| e.into_inner()).take() {
                        let _ = old_child.kill();
                        let _ = old_child.wait();
                    }
                }
            }

            if !stopped_clone.load(Ordering::SeqCst) {
                let _ = tx.send(Err(
                    "Unable to establish secure tunnel. Please use Local Wi-Fi sharing.".to_string(),
                ));
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
        if let Some(mut child) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for PinggyTunnel {
    fn drop(&mut self) {
        self.stop();
    }
}

fn extract_tunnel_url(line: &str) -> Option<String> {
    for word in line.split_whitespace() {
        let clean = word.trim_matches(|c| {
            c == '\'' || c == '"' || c == ',' || c == ';' || c == '(' || c == ')' || c == '[' || c == ']'
        });
        if clean.starts_with("https://")
            && !clean.contains("dashboard.pinggy.io")
            && clean.contains("pinggy")
        {
            return Some(clean.to_string());
        }
    }
    None
}
