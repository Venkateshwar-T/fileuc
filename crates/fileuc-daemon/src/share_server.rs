use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone, PartialEq)]
pub enum ShareServerEvent {
    ClientConnected,
    DownloadStarted,
    DownloadFinished,
    Error(String),
}

pub struct ShareServerHandle {
    pub url: String,
    pub file_name: String,
    pub file_size: u64,
    pub local_ip: IpAddr,
    pub port: u16,
    pub token: String,
    pub rx: Receiver<ShareServerEvent>,
    shutdown: Arc<AtomicBool>,
    client_streams: Arc<std::sync::Mutex<Vec<TcpStream>>>,
    thread: Option<JoinHandle<()>>,
}

impl ShareServerHandle {
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Ok(streams) = self.client_streams.lock() {
            for stream in streams.iter() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ShareServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn get_local_lan_ip() -> Option<IpAddr> {
    // 1. Query UDP routing table with well-known targets
    for probe in &["8.8.8.8:80", "1.1.1.1:80", "223.5.5.5:80"] {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(probe).is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    let ip = addr.ip();
                    if !ip.is_loopback() && !ip.is_unspecified() {
                        return Some(ip);
                    }
                }
            }
        }
    }

    // 2. Query Windows Wi-Fi adapter IPv4
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                "(Get-NetIPAddress -AddressFamily IPv4 -InterfaceAlias 'Wi-Fi*' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty IPAddress -First 1)",
            ])
            .creation_flags(0x08000000)
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(ip) = s.parse::<IpAddr>() {
                    if !ip.is_loopback() {
                        return Some(ip);
                    }
                }
            }
        }

        // 3. Fallback: query any active non-loopback IPv4 connection
        if let Ok(output) = std::process::Command::new("powershell")
            .args(&[
                "-NoProfile",
                "-Command",
                "(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' } | Select-Object -ExpandProperty IPAddress -First 1)",
            ])
            .creation_flags(0x08000000)
            .output()
        {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(ip) = s.parse::<IpAddr>() {
                    if !ip.is_loopback() {
                        return Some(ip);
                    }
                }
            }
        }
    }

    None
}

fn generate_token() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:016x}", now ^ 0x5a5a_a5a5_c3c3_3c3c)
}

pub fn start_share_server(files: &[PathBuf]) -> Result<ShareServerHandle, String> {
    if files.is_empty() {
        return Err("No files specified for sharing".to_string());
    }

    let local_ip = get_local_lan_ip().unwrap_or(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    let listener = TcpListener::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind TCP listener: {}", e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set nonblocking: {}", e))?;

    let port = listener
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("Failed to get local port: {}", e))?;

    let token = generate_token();
    let url = format!("http://{}:{}/s/{}", local_ip, port, token);

    // Prepare shared file (direct single file or zip package)
    let (target_path, display_name, file_size, temp_guard) = if files.len() == 1 {
        let path = &files[0];
        let meta = std::fs::metadata(path).map_err(|e| format!("Cannot read file: {}", e))?;
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "shared_file".to_string());
        (path.clone(), name, meta.len(), None)
    } else {
        let mut temp_file = NamedTempFile::new().map_err(|e| format!("Temp file error: {}", e))?;
        {
            let mut zip = ZipWriter::new(&mut temp_file);
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            for f in files {
                if let Ok(mut src) = File::open(f) {
                    let name = f
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".to_string());
                    if zip.start_file(&name, options).is_ok() {
                        let _ = std::io::copy(&mut src, &mut zip);
                    }
                }
            }
            zip.finish().map_err(|e| format!("Zip error: {}", e))?;
        }
        let size = temp_file.as_file().metadata().map(|m| m.len()).unwrap_or(0);
        let path = temp_file.path().to_path_buf();
        (path, "Shared_Files.zip".to_string(), size, Some(temp_file))
    };

    let (tx, rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    let token_clone = token.clone();
    let display_name_clone = display_name.clone();

    let shared_temp = Arc::new(temp_guard);
    let client_streams = Arc::new(std::sync::Mutex::new(Vec::new()));
    let client_streams_clone = client_streams.clone();

    let thread = thread::spawn(move || {
        let keep_temp = shared_temp;

        while !shutdown_clone.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    if let Ok(cloned_sock) = stream.try_clone() {
                        let _ = cloned_sock.set_nonblocking(false);
                        if let Ok(mut lock) = client_streams_clone.lock() {
                            lock.push(cloned_sock);
                        }
                    }
                    let target = target_path.clone();
                    let name = display_name_clone.clone();
                    let tok = token_clone.clone();
                    let tx_sub = tx.clone();
                    let temp_holder = keep_temp.clone();
                    thread::spawn(move || {
                        let _keep_alive = temp_holder;
                        handle_client(
                            &mut stream,
                            &target,
                            &name,
                            file_size,
                            &tok,
                            &tx_sub,
                        );
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(30));
                }
                Err(e) => {
                    let _ = tx.send(ShareServerEvent::Error(format!("Accept error: {}", e)));
                    break;
                }
            }
        }
    });

    Ok(ShareServerHandle {
        url,
        file_name: display_name,
        file_size,
        local_ip,
        port,
        token,
        rx,
        shutdown,
        client_streams,
        thread: Some(thread),
    })
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn percent_encode_rfc5987(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
        {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn parse_byte_range(spec: &str, file_size: u64) -> Option<(u64, u64)> {
    if file_size == 0 {
        return None;
    }
    let clean = spec.trim().trim_start_matches("bytes=").trim();
    if clean.starts_with('-') {
        let suffix = clean.trim_start_matches('-').parse::<u64>().ok()?;
        let start = file_size.saturating_sub(suffix);
        let end = file_size.saturating_sub(1);
        Some((start, end))
    } else {
        let parts: Vec<&str> = clean.split('-').collect();
        if parts.is_empty() {
            return None;
        }
        let start = parts[0].trim().parse::<u64>().ok()?;
        if start >= file_size {
            return None;
        }
        let end = if parts.len() > 1 && !parts[1].trim().is_empty() {
            parts[1].trim().parse::<u64>().ok()?.min(file_size.saturating_sub(1))
        } else {
            file_size.saturating_sub(1)
        };
        if start <= end {
            Some((start, end))
        } else {
            None
        }
    }
}

fn handle_client(
    stream: &mut TcpStream,
    target_path: &Path,
    display_name: &str,
    file_size: u64,
    token: &str,
    tx: &Sender<ShareServerEvent>,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(300)));

    let Ok(cloned) = stream.try_clone() else {
        return;
    };
    let _ = cloned.set_nonblocking(false);
    let mut reader = BufReader::new(cloned);
    let mut request_line = String::new();

    // Enforce 4KB max request line to eliminate Slowloris and memory amplification attacks
    let mut limited = reader.by_ref().take(4096);
    if limited.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }

    // Read remaining headers until blank line (capped at 64 headers)
    let mut range_header: Option<String> = None;
    let mut header_lines_count = 0;
    let mut header_line = String::new();
    while let Ok(n) = reader.by_ref().take(4096).read_line(&mut header_line) {
        if n == 0 || header_line == "\r\n" || header_line == "\n" {
            break;
        }
        let trimmed = header_line.trim();
        if trimmed.to_ascii_lowercase().starts_with("range:") {
            if let Some(eq_idx) = trimmed.find('=') {
                range_header = Some(trimmed[eq_idx + 1..].trim().to_string());
            }
        }
        header_lines_count += 1;
        if header_lines_count > 64 {
            return;
        }
        header_line.clear();
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let raw_path = parts[1];
    let clean_path = raw_path.split('?').next().unwrap_or(raw_path).trim_end_matches('/');

    if clean_path == "/favicon.ico" {
        let no_content = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(no_content.as_bytes());
        let _ = stream.flush();
        return;
    }

    let is_share_page = clean_path == format!("/s/{}", token) || clean_path == "/s" || clean_path.is_empty();
    if (method == "GET" || method == "HEAD") && is_share_page {
        let _ = tx.send(ShareServerEvent::ClientConnected);
        let html = generate_mobile_html(display_name, file_size, token);
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
            Content-Type: text/html; charset=utf-8\r\n\
            Content-Length: {}\r\n\
            Connection: close\r\n\
            Cache-Control: no-cache, no-store\r\n\r\n{}",
            html.len(),
            if method == "GET" { &html } else { "" }
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    } else if (method == "GET" || method == "HEAD") && clean_path.starts_with(&format!("/d/{}/", token)) {
        let _ = tx.send(ShareServerEvent::DownloadStarted);
        let ascii_fallback: String = display_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let rfc5987_filename = percent_encode_rfc5987(display_name);

        if let Some(ref range_str) = range_header {
            if let Some((start, end)) = parse_byte_range(range_str, file_size) {
                let content_length = end - start + 1;
                let header = format!(
                    "HTTP/1.1 206 Partial Content\r\n\
                    Content-Type: application/octet-stream\r\n\
                    Content-Disposition: attachment; filename=\"{}\"; filename*=UTF-8''{}\r\n\
                    Accept-Ranges: bytes\r\n\
                    Content-Range: bytes {}-{}/{}\r\n\
                    Content-Length: {}\r\n\
                    Connection: close\r\n\
                    Cache-Control: no-cache, no-store\r\n\r\n",
                    ascii_fallback, rfc5987_filename, start, end, file_size, content_length
                );
                if stream.write_all(header.as_bytes()).is_ok() && method == "GET" {
                    if let Ok(mut file) = File::open(target_path) {
                        let _ = file.seek(SeekFrom::Start(start));
                        let mut take_stream = file.take(content_length);
                        let _ = std::io::copy(&mut take_stream, stream);
                        let _ = stream.flush();
                        if end + 1 >= file_size {
                            let _ = tx.send(ShareServerEvent::DownloadFinished);
                        }
                    }
                }
            } else {
                let err_resp = format!(
                    "HTTP/1.1 416 Range Not Satisfiable\r\n\
                    Content-Range: bytes */{}\r\n\
                    Content-Length: 0\r\n\
                    Connection: close\r\n\r\n",
                    file_size
                );
                let _ = stream.write_all(err_resp.as_bytes());
                let _ = stream.flush();
            }
        } else {
            // Full 200 OK response with Accept-Ranges: bytes
            if let Ok(mut file) = File::open(target_path) {
                let header = format!(
                    "HTTP/1.1 200 OK\r\n\
                    Content-Type: application/octet-stream\r\n\
                    Content-Disposition: attachment; filename=\"{}\"; filename*=UTF-8''{}\r\n\
                    Accept-Ranges: bytes\r\n\
                    Content-Length: {}\r\n\
                    Connection: close\r\n\
                    Cache-Control: no-cache, no-store\r\n\r\n",
                    ascii_fallback, rfc5987_filename, file_size
                );
                if stream.write_all(header.as_bytes()).is_ok() && method == "GET" {
                    let _ = std::io::copy(&mut file, stream);
                    let _ = stream.flush();
                    let _ = tx.send(ShareServerEvent::DownloadFinished);
                }
            } else {
                let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(not_found.as_bytes());
                let _ = stream.flush();
            }
        }
    } else if method == "POST" && clean_path == format!("/api/notify/{}", token) {
        let _ = tx.send(ShareServerEvent::DownloadStarted);
        let json = "{\"status\":\"ok\"}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            json.len(),
            json
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    } else {
        let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(not_found.as_bytes());
        let _ = stream.flush();
    }

    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn generate_mobile_html(file_name: &str, file_size: u64, token: &str) -> String {
    let size_str = format_size(file_size);
    let escaped_name = escape_html(file_name);
    let download_url = format!("/d/{}/{}", token, percent_encode_rfc5987(file_name));

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
<title>Download {escaped_name} • FileUC</title>
<style>
:root {{
  --bg: #F5F6F8;
  --card: #FFFFFF;
  --text: #111827;
  --sub: #6B7280;
  --border: #E5E7EB;
  --primary: #7066E0;
  --primary-hover: #5E54D0;
  --accent-light: rgba(112, 102, 224, 0.12);
  --success: #10B981;
}}
@media (prefers-color-scheme: dark) {{
  :root {{
    --bg: #0F172A;
    --card: #1E293B;
    --text: #F8FAFC;
    --sub: #94A3B8;
    --border: #334155;
    --primary: #7066E0;
    --primary-hover: #8177EC;
    --accent-light: rgba(112, 102, 224, 0.20);
    --success: #10B981;
  }}
}}
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{
  background: var(--bg);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: 20px;
}}
.card {{
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 24px;
  padding: 32px 24px;
  width: 100%;
  max-width: 380px;
  text-align: center;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.08), 0 8px 10px -6px rgba(0, 0, 0, 0.03);
}}
.icon-box {{
  width: 68px;
  height: 68px;
  background: var(--accent-light);
  border-radius: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 20px;
  color: var(--primary);
}}
.icon-box svg {{ width: 34px; height: 34px; }}
h1 {{
  font-size: 19px;
  font-weight: 700;
  margin-bottom: 6px;
  word-break: break-word;
  line-height: 1.35;
}}
.meta {{
  font-size: 14px;
  color: var(--sub);
  margin-bottom: 28px;
}}
.btn {{
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 100%;
  background: var(--primary);
  color: #FFFFFF;
  text-decoration: none;
  font-size: 16px;
  font-weight: 600;
  padding: 16px;
  border-radius: 14px;
  border: none;
  cursor: pointer;
  transition: transform 0.15s ease, background 0.15s ease;
  box-shadow: 0 4px 14px rgba(112, 102, 224, 0.30);
}}
.btn:active {{
  transform: scale(0.98);
  background: var(--primary-hover);
}}
.btn svg {{ width: 20px; height: 20px; }}
.footer {{
  margin-top: 24px;
  font-size: 12px;
  color: var(--sub);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}}
.status-pill {{
  display: none;
  margin-top: 16px;
  padding: 10px;
  background: rgba(16, 185, 129, 0.12);
  color: var(--success);
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
}}
</style>
</head>
<body>
<div class="card">
  <div class="icon-box">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
    </svg>
  </div>
  <h1>{escaped_name}</h1>
  <div class="meta">{size_str} • Direct Local Transfer</div>
  <a id="dl-btn" href="{download_url}" class="btn" onclick="startDownload()">
    <svg fill="none" stroke="currentColor" stroke-width="2.2" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"></path>
    </svg>
    Download to Device
  </a>
  <div id="status-pill" class="status-pill">✓ Download Initiated! Check Notifications</div>
  <div class="footer">
    <span>Transferred with FileUC</span>
  </div>
</div>
<script>
function startDownload() {{
  const pill = document.getElementById('status-pill');
  if (pill) pill.style.display = 'block';
  fetch('/api/notify/{token}', {{ method: 'POST' }}).catch(() => {{}});
}}
</script>
</body>
</html>"#, escaped_name = escaped_name, size_str = size_str, download_url = download_url, token = token)
}
