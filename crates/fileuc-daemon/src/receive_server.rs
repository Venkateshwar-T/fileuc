use crate::clipboard;
use crate::share_server::{bind_local_listener, constant_time_eq, escape_html, generate_token};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::net::{IpAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_CONCURRENT_CLIENTS: usize = 4;
const MAX_UPLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_FILES_PER_REQUEST: usize = 100;
const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const STREAM_BUF_SIZE: usize = 64 * 1024;
const FREE_SPACE_SAFETY_MARGIN: u64 = 64 * 1024 * 1024;

static PART_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveServerEvent {
    ClientConnected,
    UploadProgress { percent: u8 },
    FilesReceived { count: usize, first_path: PathBuf },
    TextReceived,
    Error(String),
}

pub struct ReceiveServerHandle {
    pub url: String,
    pub save_dir: PathBuf,
    pub local_ip: IpAddr,
    pub port: u16,
    pub token: String,
    pub rx: Receiver<ReceiveServerEvent>,
    shutdown: Arc<AtomicBool>,
    client_streams: Arc<Mutex<HashMap<u64, TcpStream>>>,
    thread: Option<JoinHandle<()>>,
}

impl ReceiveServerHandle {
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Ok(mut streams) = self.client_streams.lock() {
            for (_, stream) in streams.drain() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for ReceiveServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_receive_server(save_dir: PathBuf) -> Result<ReceiveServerHandle, String> {
    fs::create_dir_all(&save_dir)
        .map_err(|e| format!("Cannot create receive folder '{}': {}", save_dir.display(), e))?;

    let (local_ip, listener, port) = bind_local_listener()?;
    let token = generate_token();
    let url = format!("http://{}:{}/r/{}", local_ip, port, token);

    let (tx, rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let token_clone = token.clone();
    let save_dir_clone = save_dir.clone();

    let client_streams = Arc::new(Mutex::new(HashMap::<u64, TcpStream>::new()));
    let client_streams_clone = client_streams.clone();
    let stream_id_counter = Arc::new(AtomicU64::new(1));
    let active_connections = Arc::new(AtomicUsize::new(0));

    let thread = thread::spawn(move || {
        while !shutdown_clone.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    if active_connections.load(Ordering::Relaxed) >= MAX_CONCURRENT_CLIENTS {
                        let _ = send_http_text(
                            &mut stream,
                            "503 Service Unavailable",
                            "text/plain; charset=utf-8",
                            "Server Busy",
                        );
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        continue;
                    }

                    active_connections.fetch_add(1, Ordering::Relaxed);
                    let stream_id = stream_id_counter.fetch_add(1, Ordering::Relaxed);
                    let _ = stream.set_nonblocking(false);
                    if let Ok(cloned_sock) = stream.try_clone() {
                        let _ = cloned_sock.set_nonblocking(false);
                        if let Ok(mut lock) = client_streams_clone.lock() {
                            lock.insert(stream_id, cloned_sock);
                        }
                    }

                    let tok = token_clone.clone();
                    let dir = save_dir_clone.clone();
                    let tx_sub = tx.clone();
                    let streams_for_worker = client_streams_clone.clone();
                    let counter_for_worker = active_connections.clone();

                    thread::spawn(move || {
                        struct StreamGuard {
                            id: u64,
                            streams: Arc<Mutex<HashMap<u64, TcpStream>>>,
                            counter: Arc<AtomicUsize>,
                        }
                        impl Drop for StreamGuard {
                            fn drop(&mut self) {
                                self.counter.fetch_sub(1, Ordering::Relaxed);
                                if let Ok(mut map) = self.streams.lock() {
                                    map.remove(&self.id);
                                }
                            }
                        }

                        let _guard = StreamGuard {
                            id: stream_id,
                            streams: streams_for_worker,
                            counter: counter_for_worker,
                        };

                        handle_client(&mut stream, &dir, &tok, &tx_sub);
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(30));
                }
                Err(e) => {
                    let _ = tx.send(ReceiveServerEvent::Error(format!("Accept error: {}", e)));
                    break;
                }
            }
        }
    });

    Ok(ReceiveServerHandle {
        url,
        save_dir,
        local_ip,
        port,
        token,
        rx,
        shutdown,
        client_streams,
        thread: Some(thread),
    })
}

fn send_http_text(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-cache, no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

fn read_http_head(stream: &mut TcpStream) -> Option<(String, HashMap<String, String>, Vec<u8>)> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut raw = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let mut header_end = None;

    while raw.len() < MAX_HTTP_HEADER_BYTES {
        if Instant::now() > deadline {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if let Some(pos) = find_subsequence(&raw, b"\r\n\r\n") {
                    header_end = Some((pos, 4));
                    break;
                }
                if let Some(pos) = find_subsequence(&raw, b"\n\n") {
                    header_end = Some((pos, 2));
                    break;
                }
            }
            Err(_) => return None,
        }
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));

    let (end_pos, sep_len) = header_end?;
    let head_str = std::str::from_utf8(&raw[..end_pos]).ok()?;
    let leftover = raw[end_pos + sep_len..].to_vec();

    let mut lines = head_str.lines();
    let request_line = lines.next()?.trim().to_string();
    let mut headers = HashMap::new();
    for line in lines.take(64) {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    Some((request_line, headers, leftover))
}

fn handle_client(
    stream: &mut TcpStream,
    save_dir: &Path,
    token: &str,
    tx: &Sender<ReceiveServerEvent>,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let Some((request_line, headers, leftover)) = read_http_head(stream) else {
        return;
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let raw_path = parts[1];
    let clean_path = raw_path.split('?').next().unwrap_or(raw_path).trim_end_matches('/');

    if clean_path == "/favicon.ico" {
        let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Both);
        return;
    }

    if (method == "GET" || method == "HEAD") && clean_path.starts_with("/r/") {
        let req_tok = clean_path.strip_prefix("/r/").unwrap_or("");
        if !constant_time_eq(req_tok, token) {
            let _ = send_http_text(stream, "403 Forbidden", "text/plain; charset=utf-8", "Forbidden");
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        }
        let _ = tx.send(ReceiveServerEvent::ClientConnected);
        let html = generate_receive_html(save_dir, token);
        let header = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             Cache-Control: no-cache, no-store\r\n\r\n",
            html.len()
        );
        if stream.write_all(header.as_bytes()).is_ok() && method == "GET" {
            let _ = stream.write_all(html.as_bytes());
        }
        let _ = stream.flush();
    } else if method == "POST" && clean_path.starts_with("/upload/") {
        let req_tok = clean_path.strip_prefix("/upload/").unwrap_or("");
        if !constant_time_eq(req_tok, token) {
            let _ = send_http_text(stream, "403 Forbidden", "text/plain; charset=utf-8", "Forbidden");
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        }
        handle_multipart_upload(stream, &headers, leftover, save_dir, tx);
    } else if method == "POST" && clean_path.starts_with("/text/") {
        let req_tok = clean_path.strip_prefix("/text/").unwrap_or("");
        if !constant_time_eq(req_tok, token) {
            let _ = send_http_text(stream, "403 Forbidden", "text/plain; charset=utf-8", "Forbidden");
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        }
        handle_text_receive(stream, &headers, leftover, tx);
    } else if clean_path.starts_with("/r") || clean_path.starts_with("/upload") || clean_path.starts_with("/text") {
        let _ = send_http_text(stream, "403 Forbidden", "text/plain; charset=utf-8", "Forbidden");
    } else {
        let _ = send_http_text(stream, "404 Not Found", "text/plain; charset=utf-8", "Not Found");
    }

    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn handle_text_receive(
    stream: &mut TcpStream,
    headers: &HashMap<String, String>,
    leftover: Vec<u8>,
    tx: &Sender<ReceiveServerEvent>,
) {
    let Some(content_len) = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
    else {
        let _ = send_http_text(
            stream,
            "411 Length Required",
            "application/json",
            "{\"error\":\"Content-Length required\"}",
        );
        return;
    };

    if content_len == 0 {
        let _ = send_http_text(
            stream,
            "400 Bad Request",
            "application/json",
            "{\"error\":\"Text cannot be empty\"}",
        );
        return;
    }

    if content_len > MAX_TEXT_BYTES {
        let _ = send_http_text(
            stream,
            "413 Payload Too Large",
            "application/json",
            "{\"error\":\"Text exceeds 256 KB limit\"}",
        );
        return;
    }

    let mut body = Vec::with_capacity(content_len);
    let take_from_leftover = content_len.min(leftover.len());
    body.extend_from_slice(&leftover[..take_from_leftover]);

    while body.len() < content_len {
        let mut tmp = [0u8; 4096];
        let to_read = (content_len - body.len()).min(tmp.len());
        match stream.read(&mut tmp[..to_read]) {
            Ok(0) => {
                let _ = send_http_text(
                    stream,
                    "400 Bad Request",
                    "application/json",
                    "{\"error\":\"Incomplete request body\"}",
                );
                return;
            }
            Ok(n) => body.extend_from_slice(&tmp[..n]),
            Err(_) => {
                let _ = send_http_text(
                    stream,
                    "400 Bad Request",
                    "application/json",
                    "{\"error\":\"Read failed\"}",
                );
                return;
            }
        }
    }

    let Ok(text) = String::from_utf8(body) else {
        let _ = send_http_text(
            stream,
            "400 Bad Request",
            "application/json",
            "{\"error\":\"Invalid UTF-8 text\"}",
        );
        return;
    };

    if text.trim().is_empty() {
        let _ = send_http_text(
            stream,
            "400 Bad Request",
            "application/json",
            "{\"error\":\"Text cannot be whitespace only\"}",
        );
        return;
    }

    if !clipboard::set_clipboard_text(&text) {
        let _ = send_http_text(
            stream,
            "500 Internal Server Error",
            "application/json",
            "{\"error\":\"Failed to update Windows clipboard\"}",
        );
        return;
    }

    let _ = tx.send(ReceiveServerEvent::TextReceived);
    let _ = send_http_text(stream, "200 OK", "application/json", "{\"ok\":true}");
}

struct PartFileGuard {
    part_path: PathBuf,
    writer: Option<BufWriter<File>>,
    committed: bool,
}

impl PartFileGuard {
    fn create_temp(dir: &Path) -> Result<Self, String> {
        let seq = PART_SEQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let part_path = dir.join(format!(".fileuc_recv_{}_{}.part", pid, seq));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part_path)
            .map_err(|e| format!("Failed to create temp part file: {}", e))?;
        Ok(Self {
            part_path,
            writer: Some(BufWriter::with_capacity(STREAM_BUF_SIZE, file)),
            committed: false,
        })
    }

    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if let Some(ref mut w) = self.writer {
            w.write_all(bytes)?;
        }
        Ok(())
    }

    fn commit_to(mut self, dir: &Path, safe_filename: &str) -> Result<PathBuf, String> {
        if let Some(mut w) = self.writer.take() {
            w.flush().map_err(|e| format!("Flush error: {}", e))?;
        }
        for attempt in 0..1000u32 {
            let candidate = numbered_candidate_path(dir, safe_filename, attempt);
            if !candidate.exists() {
                match fs::rename(&self.part_path, &candidate) {
                    Ok(()) => {
                        self.committed = true;
                        return Ok(candidate);
                    }
                    Err(e) if candidate.exists() => {
                        let _ = e;
                        continue;
                    }
                    Err(e) => {
                        return Err(format!("Failed to finalize file: {}", e));
                    }
                }
            }
        }
        Err("Too many filename collisions in receive folder".to_string())
    }
}

impl Drop for PartFileGuard {
    fn drop(&mut self) {
        self.writer.take();
        if !self.committed {
            let _ = fs::remove_file(&self.part_path);
        }
    }
}

fn handle_multipart_upload(
    stream: &mut TcpStream,
    headers: &HashMap<String, String>,
    leftover: Vec<u8>,
    save_dir: &Path,
    tx: &Sender<ReceiveServerEvent>,
) {
    let Some(content_type) = headers.get("content-type") else {
        let _ = send_http_text(
            stream,
            "400 Bad Request",
            "application/json",
            "{\"error\":\"Missing Content-Type\"}",
        );
        return;
    };

    let Some(boundary) = extract_boundary(content_type) else {
        let _ = send_http_text(
            stream,
            "400 Bad Request",
            "application/json",
            "{\"error\":\"Missing multipart boundary\"}",
        );
        return;
    };

    let expected_len = headers
        .get("content-length")
        .and_then(|v| v.parse::<u64>().ok());

    if let Some(len) = expected_len {
        if len > MAX_UPLOAD_BYTES {
            let _ = send_http_text(
                stream,
                "413 Payload Too Large",
                "application/json",
                "{\"error\":\"Upload exceeds 4 GB limit\"}",
            );
            return;
        }
        if let Some(free) = available_disk_bytes(save_dir) {
            if len.saturating_add(FREE_SPACE_SAFETY_MARGIN) > free {
                let _ = send_http_text(
                    stream,
                    "507 Insufficient Storage",
                    "application/json",
                    "{\"error\":\"Not enough free disk space\"}",
                );
                return;
            }
        }
    }

    let _ = fs::create_dir_all(save_dir);
    let mut reader = StreamSource::new(stream, leftover, expected_len);

    match stream_multipart_parts(&mut reader, &boundary, save_dir, expected_len, tx) {
        Ok(saved_paths) => {
            if saved_paths.is_empty() {
                let _ = send_http_text(
                    stream,
                    "400 Bad Request",
                    "application/json",
                    "{\"error\":\"No files found in upload\"}",
                );
                return;
            }
            let count = saved_paths.len();
            let first_path = saved_paths[0].clone();
            let _ = tx.send(ReceiveServerEvent::FilesReceived { count, first_path });
            let json = format!("{{\"ok\":true,\"count\":{}}}", count);
            let _ = send_http_text(stream, "200 OK", "application/json", &json);
        }
        Err(UploadError::PayloadTooLarge) => {
            let _ = send_http_text(
                stream,
                "413 Payload Too Large",
                "application/json",
                "{\"error\":\"Upload exceeds size or file count limit\"}",
            );
        }
        Err(UploadError::BadRequest(msg)) => {
            let json = format!("{{\"error\":\"{}\"}}", escape_json(&msg));
            let _ = send_http_text(stream, "400 Bad Request", "application/json", &json);
        }
        Err(UploadError::Io(msg)) => {
            let _ = tx.send(ReceiveServerEvent::Error(msg.clone()));
            let json = format!("{{\"error\":\"{}\"}}", escape_json(&msg));
            let _ = send_http_text(
                stream,
                "500 Internal Server Error",
                "application/json",
                &json,
            );
        }
    }
}

enum UploadError {
    PayloadTooLarge,
    BadRequest(String),
    Io(String),
}

struct StreamSource<'a> {
    stream: &'a mut TcpStream,
    buf: Vec<u8>,
    bytes_received: u64,
    max_bytes: u64,
    eof: bool,
}

impl<'a> StreamSource<'a> {
    fn new(stream: &'a mut TcpStream, initial: Vec<u8>, expected_len: Option<u64>) -> Self {
        let initial_len = initial.len() as u64;
        Self {
            stream,
            buf: initial,
            bytes_received: initial_len,
            max_bytes: expected_len.unwrap_or(MAX_UPLOAD_BYTES).min(MAX_UPLOAD_BYTES),
            eof: false,
        }
    }

    fn fill(&mut self) -> Result<usize, UploadError> {
        if self.eof {
            return Ok(0);
        }
        if self.bytes_received >= self.max_bytes {
            self.eof = true;
            return Ok(0);
        }

        let mut tmp = [0u8; STREAM_BUF_SIZE];
        match self.stream.read(&mut tmp) {
            Ok(0) => {
                self.eof = true;
                Ok(0)
            }
            Ok(n) => {
                let next_total = self.bytes_received.saturating_add(n as u64);
                if next_total > MAX_UPLOAD_BYTES {
                    return Err(UploadError::PayloadTooLarge);
                }
                let allowed = ((self.max_bytes - self.bytes_received) as usize).min(n);
                self.buf.extend_from_slice(&tmp[..allowed]);
                self.bytes_received += allowed as u64;
                Ok(allowed)
            }
            Err(e) => Err(UploadError::Io(format!("Network read error: {}", e))),
        }
    }
}

fn stream_multipart_parts(
    src: &mut StreamSource<'_>,
    boundary: &str,
    save_dir: &Path,
    expected_len: Option<u64>,
    tx: &Sender<ReceiveServerEvent>,
) -> Result<Vec<PathBuf>, UploadError> {
    let first_marker = format!("--{}", boundary).into_bytes();
    let part_marker = format!("\r\n--{}", boundary).into_bytes();

    // 1. Seek initial boundary `--{boundary}`
    loop {
        if let Some(pos) = find_subsequence(&src.buf, &first_marker) {
            let after = pos + first_marker.len();
            while src.buf.len() < after + 2 && !src.eof {
                src.fill()?;
            }
            if src.buf.len() >= after + 2 && &src.buf[after..after + 2] == b"--" {
                return Ok(Vec::new());
            }
            if src.buf.len() >= after + 2 && &src.buf[after..after + 2] == b"\r\n" {
                src.buf.drain(..after + 2);
                break;
            }
            if src.buf.len() >= after + 1 && src.buf[after] == b'\n' {
                src.buf.drain(..after + 1);
                break;
            }
            return Err(UploadError::BadRequest("Malformed multipart boundary".into()));
        }
        if src.buf.len() > 64 * 1024 {
            return Err(UploadError::BadRequest("Invalid multipart preamble".into()));
        }
        if src.fill()? == 0 {
            return Err(UploadError::BadRequest("Missing initial multipart boundary".into()));
        }
    }

    let mut saved = Vec::new();
    let mut last_pct: u8 = 0;
    let mut last_progress_emit = Instant::now();

    loop {
        // 2. Read part headers up to `\r\n\r\n`
        let header_end = loop {
            if let Some(pos) = find_subsequence(&src.buf, b"\r\n\r\n") {
                break (pos, 4);
            }
            if src.buf.len() > 8 * 1024 {
                return Err(UploadError::BadRequest("Part headers too large".into()));
            }
            if src.fill()? == 0 {
                return Err(UploadError::BadRequest("Unexpected EOF in part headers".into()));
            }
        };

        let header_bytes = src.buf[..header_end.0].to_vec();
        src.buf.drain(..header_end.0 + header_end.1);
        let header_text = String::from_utf8_lossy(&header_bytes);
        let maybe_filename = extract_part_filename(&header_text);

        let mut guard = if let Some(ref raw_name) = maybe_filename {
            if saved.len() >= MAX_FILES_PER_REQUEST {
                return Err(UploadError::PayloadTooLarge);
            }
            let safe_name = sanitize_filename(raw_name);
            Some((
                safe_name,
                PartFileGuard::create_temp(save_dir).map_err(UploadError::Io)?,
            ))
        } else {
            None
        };

        // 3. Stream part payload until next `\r\n--{boundary}`
        let is_last_part = loop {
            if let Some(pos) = find_subsequence(&src.buf, &part_marker) {
                if let Some((_, ref mut pf)) = guard {
                    pf.write_all(&src.buf[..pos])
                        .map_err(|e| UploadError::Io(format!("Disk write error: {}", e)))?;
                }
                let after = pos + part_marker.len();
                while src.buf.len() < after + 2 && !src.eof {
                    src.fill()?;
                }
                let end_reached =
                    src.buf.len() >= after + 2 && &src.buf[after..after + 2] == b"--";
                let consume = if end_reached {
                    after + 2
                } else if src.buf.len() >= after + 2 && &src.buf[after..after + 2] == b"\r\n" {
                    after + 2
                } else if src.buf.len() >= after + 1 && src.buf[after] == b'\n' {
                    after + 1
                } else {
                    after
                };
                src.buf.drain(..consume.min(src.buf.len()));

                if let Some((safe_name, pf)) = guard.take() {
                    let final_path = pf.commit_to(save_dir, &safe_name).map_err(UploadError::Io)?;
                    saved.push(final_path);
                }
                break end_reached;
            }

            // Safely flush everything except the tail that could contain a split boundary
            let keep_tail = part_marker.len() + 4;
            if src.buf.len() > keep_tail {
                let flush_len = src.buf.len() - keep_tail;
                if let Some((_, ref mut pf)) = guard {
                    pf.write_all(&src.buf[..flush_len])
                        .map_err(|e| UploadError::Io(format!("Disk write error: {}", e)))?;
                }
                src.buf.drain(..flush_len);
            }

            if let Some(total) = expected_len {
                if total > 0 {
                    let pct = ((src.bytes_received.saturating_mul(100)) / total).min(99) as u8;
                    if pct >= last_pct.saturating_add(2)
                        || last_progress_emit.elapsed() >= Duration::from_millis(250)
                    {
                        last_pct = pct;
                        last_progress_emit = Instant::now();
                        let _ = tx.send(ReceiveServerEvent::UploadProgress { percent: pct });
                    }
                }
            }

            if src.fill()? == 0 {
                return Err(UploadError::BadRequest(
                    "Upload ended before multipart terminator".into(),
                ));
            }
        };

        if is_last_part {
            let _ = tx.send(ReceiveServerEvent::UploadProgress { percent: 100 });
            break;
        }
    }

    Ok(saved)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn extract_boundary(content_type: &str) -> Option<String> {
    if !content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        return None;
    }
    for segment in content_type.split(';') {
        let s = segment.trim();
        if let Some((k, v)) = s.split_once('=') {
            if k.trim().eq_ignore_ascii_case("boundary") {
                let b = v.trim().trim_matches('"').trim();
                if !b.is_empty() && b.len() <= 200 {
                    return Some(b.to_string());
                }
            }
        }
    }
    None
}

fn extract_part_filename(headers_block: &str) -> Option<String> {
    for line in headers_block.lines() {
        if line.to_ascii_lowercase().starts_with("content-disposition:") {
            // Prefer filename="..."
            for part in line.split(';') {
                let p = part.trim();
                if let Some((k, v)) = p.split_once('=') {
                    if k.trim().eq_ignore_ascii_case("filename") {
                        let name = v.trim().trim_matches('"');
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn sanitize_filename(raw: &str) -> String {
    let base = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim();

    let mut cleaned = String::with_capacity(base.len());
    for c in base.chars() {
        if c.is_control() {
            continue;
        }
        match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => cleaned.push('_'),
            _ => cleaned.push(c),
        }
    }

    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_string();
    let cleaned = if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "uploaded_file".to_string()
    } else {
        cleaned
    };

    // Split stem and extension (respect Windows reserved device names)
    let (stem, ext) = match cleaned.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{}", e)),
        _ => (cleaned, String::new()),
    };

    // Check first dot-segment against Windows reserved device names (e.g. CON.tar.gz)
    let first_segment = stem.split('.').next().unwrap_or(&stem);
    let is_reserved = matches!(
        first_segment.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );

    let safe_stem = if is_reserved {
        format!("_{}", stem)
    } else {
        stem
    };

    let truncated_stem = truncate_utf8_bytes(&safe_stem, 160);
    let truncated_ext = truncate_utf8_bytes(&ext, 24);
    let final_name = format!("{}{}", truncated_stem, truncated_ext);
    if final_name.is_empty() {
        "uploaded_file".to_string()
    } else {
        final_name
    }
}

fn truncate_utf8_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}

fn numbered_candidate_path(dir: &Path, safe_filename: &str, attempt: u32) -> PathBuf {
    if attempt == 0 {
        return dir.join(safe_filename);
    }
    match safe_filename.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            dir.join(format!("{} ({}).{}", stem, attempt, ext))
        }
        _ => dir.join(format!("{} ({})", safe_filename, attempt)),
    }
}

fn available_disk_bytes(dir: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let mut target = dir;
        while !target.exists() {
            target = target.parent()?;
        }
        let wide: Vec<u16> = target
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free_avail = 0u64;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(wide.as_ptr()),
                Some(&mut free_avail),
                None,
                None,
            )
            .is_ok()
        };
        if ok {
            Some(free_avail)
        } else {
            None
        }
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        None
    }
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

fn generate_receive_html(save_dir: &Path, token: &str) -> String {
    let folder_label = save_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "FileUC Received".to_string());
    let escaped_folder = escape_html(&folder_label);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
<title>Send to PC • FileUC</title>
<style>
:root {{
  --bg: #F5F6F8;
  --card: #FFFFFF;
  --surface: #F8FAFC;
  --text: #111827;
  --sub: #6B7280;
  --border: #E5E7EB;
  --primary: #7066E0;
  --primary-hover: #5E54D0;
  --accent-light: rgba(112, 102, 224, 0.12);
  --success: #10B981;
  --danger: #EF4444;
}}
@media (prefers-color-scheme: dark) {{
  :root {{
    --bg: #0F172A;
    --card: #1E293B;
    --surface: #0F172A;
    --text: #F8FAFC;
    --sub: #94A3B8;
    --border: #334155;
    --primary: #7066E0;
    --primary-hover: #8177EC;
    --accent-light: rgba(112, 102, 224, 0.20);
    --success: #10B981;
    --danger: #F87171;
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
  padding: 28px 22px;
  width: 100%;
  max-width: 420px;
  text-align: center;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.08), 0 8px 10px -6px rgba(0, 0, 0, 0.03);
}}
.icon-box {{
  width: 60px;
  height: 60px;
  background: var(--accent-light);
  border-radius: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 14px;
  color: var(--primary);
}}
.icon-box svg {{ width: 30px; height: 30px; }}
h1 {{ font-size: 20px; font-weight: 700; margin-bottom: 4px; }}
.meta {{ font-size: 13px; color: var(--sub); margin-bottom: 20px; }}
.btn {{
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  background: var(--primary);
  color: #FFFFFF;
  font-size: 15.5px;
  font-weight: 600;
  padding: 15px;
  border-radius: 14px;
  border: none;
  cursor: pointer;
  transition: transform 0.15s ease, background 0.15s ease;
  box-shadow: 0 4px 14px rgba(112, 102, 224, 0.28);
}}
.btn:active {{ transform: scale(0.98); background: var(--primary-hover); }}
.btn:disabled {{ opacity: 0.6; cursor: not-allowed; }}
.progress-wrap {{
  display: none;
  margin-top: 14px;
  text-align: left;
}}
.progress-label {{
  display: flex;
  justify-content: space-between;
  font-size: 12.5px;
  color: var(--sub);
  margin-bottom: 6px;
}}
.progress-track {{
  width: 100%;
  height: 8px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 999px;
  overflow: hidden;
}}
.progress-fill {{
  height: 100%;
  width: 0%;
  background: var(--primary);
  transition: width 0.15s linear;
}}
.divider {{
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 22px 0 16px;
  font-size: 12px;
  color: var(--sub);
}}
.divider::before, .divider::after {{
  content: "";
  flex: 1;
  height: 1px;
  background: var(--border);
}}
.note-box {{
  width: 100%;
  min-height: 76px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px;
  font-family: inherit;
  font-size: 14px;
  resize: vertical;
  margin-bottom: 10px;
}}
.btn-secondary {{
  width: 100%;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}}
.status-pill {{
  display: none;
  margin-top: 14px;
  padding: 10px;
  border-radius: 10px;
  font-size: 13.5px;
  font-weight: 600;
}}
.status-ok {{
  background: rgba(16, 185, 129, 0.12);
  color: var(--success);
}}
.status-err {{
  background: rgba(239, 68, 68, 0.12);
  color: var(--danger);
}}
.footer {{ margin-top: 18px; font-size: 12px; color: var(--sub); }}
</style>
</head>
<body>
<div class="card">
  <div class="icon-box">
    <svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"></path>
    </svg>
  </div>
  <h1>Send to PC</h1>
  <div class="meta">Saving to {escaped_folder}</div>

  <input id="file-input" type="file" multiple style="display:none">
  <button id="pick-btn" type="button" class="btn" onclick="document.getElementById('file-input').click()">
    Select Files / Photos
  </button>

  <div id="progress-wrap" class="progress-wrap">
    <div class="progress-label">
      <span id="progress-text">Uploading...</span>
      <span id="progress-pct">0%</span>
    </div>
    <div class="progress-track">
      <div id="progress-fill" class="progress-fill"></div>
    </div>
  </div>

  <div class="divider">or send a quick note / link</div>

  <textarea id="note-input" class="note-box" placeholder="Paste a link or quick note to send to your PC clipboard..."></textarea>
  <button id="note-btn" type="button" class="btn-secondary" onclick="sendNote()">Send Text to PC</button>

  <div id="status-pill" class="status-pill"></div>
  <div class="footer">Receiving via FileUC</div>
</div>
<script>
const TOKEN = "{token}";
const fileInput = document.getElementById('file-input');
const pickBtn = document.getElementById('pick-btn');
const noteBtn = document.getElementById('note-btn');
const progressWrap = document.getElementById('progress-wrap');
const progressFill = document.getElementById('progress-fill');
const progressText = document.getElementById('progress-text');
const progressPct = document.getElementById('progress-pct');
const statusPill = document.getElementById('status-pill');

function showStatus(msg, ok) {{
  statusPill.textContent = msg;
  statusPill.className = 'status-pill ' + (ok ? 'status-ok' : 'status-err');
  statusPill.style.display = 'block';
}}

fileInput.addEventListener('change', () => {{
  const files = fileInput.files;
  if (!files || files.length === 0) return;

  const fileCount = files.length;
  const formData = new FormData();
  for (let i = 0; i < fileCount; i++) {{
    formData.append('files', files[i], files[i].name);
  }}
  fileInput.value = '';

  pickBtn.disabled = true;
  statusPill.style.display = 'none';
  progressWrap.style.display = 'block';
  progressFill.style.width = '0%';
  progressPct.textContent = '0%';
  progressText.textContent = 'Sending ' + fileCount + (fileCount === 1 ? ' file...' : ' files...');

  const xhr = new XMLHttpRequest();
  xhr.open('POST', '/upload/' + TOKEN, true);
  xhr.upload.onprogress = (e) => {{
    if (e.lengthComputable && e.total > 0) {{
      const pct = Math.min(100, Math.round((e.loaded * 100) / e.total));
      progressFill.style.width = pct + '%';
      progressPct.textContent = pct + '%';
    }}
  }};
  xhr.onload = () => {{
    pickBtn.disabled = false;
    if (xhr.status >= 200 && xhr.status < 300) {{
      let savedCount = fileCount;
      try {{
        const data = JSON.parse(xhr.responseText);
        if (data && typeof data.count === 'number' && data.count > 0) {{
          savedCount = data.count;
        }}
      }} catch {{}}
      progressFill.style.width = '100%';
      progressPct.textContent = '100%';
      progressText.textContent = 'Sent ' + savedCount + (savedCount === 1 ? ' file' : ' files');
      showStatus('✓ Sent ' + savedCount + (savedCount === 1 ? ' file' : ' files') + ' to PC!', true);
    }} else {{
      progressText.textContent = 'Upload failed';
      let errMsg = 'Upload failed (' + xhr.status + ')';
      try {{
        const data = JSON.parse(xhr.responseText);
        if (data && data.error) errMsg = data.error;
      }} catch {{}}
      showStatus(errMsg, false);
    }}
  }};
  xhr.onerror = () => {{
    pickBtn.disabled = false;
    progressText.textContent = 'Upload failed';
    showStatus('Connection lost during upload', false);
  }};
  xhr.send(formData);
}});

async function sendNote() {{
  const ta = document.getElementById('note-input');
  const text = (ta.value || '').trim();
  if (!text) {{
    showStatus('Enter some text or a link first', false);
    return;
  }}
  noteBtn.disabled = true;
  try {{
    const res = await fetch('/text/' + TOKEN, {{
      method: 'POST',
      headers: {{ 'Content-Type': 'text/plain; charset=utf-8' }},
      body: ta.value
    }});
    if (res.ok) {{
      ta.value = '';
      showStatus('✓ Copied to PC Clipboard!', true);
    }} else {{
      let errMsg = 'Failed to send text';
      try {{
        const d = await res.json();
        if (d && d.error) errMsg = d.error;
      }} catch {{}}
      showStatus(errMsg, false);
    }}
  }} catch {{
    showStatus('Connection failed', false);
  }} finally {{
    noteBtn.disabled = false;
  }}
}}
</script>
</body>
</html>"#,
        escaped_folder = escaped_folder,
        token = token
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_security() {
        assert_eq!(sanitize_filename("../../evil/..\\secret.txt"), "secret.txt");
        assert_eq!(sanitize_filename("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_filename("CON.tar.gz"), "_CON.tar.gz");
        assert_eq!(sanitize_filename("nul"), "_nul");
        assert_eq!(sanitize_filename("a<b>c:d\"e|f?g*.png"), "a_b_c_d_e_f_g_.png");
        assert_eq!(sanitize_filename("   ...  "), "uploaded_file");
        assert_eq!(sanitize_filename("note.txt...   "), "note.txt");
    }

    #[test]
    fn test_receive_server_auth_upload_collision_and_partial_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let mut handle = start_receive_server(tmp.path().to_path_buf()).unwrap();
        let addr = format!("127.0.0.1:{}", handle.port);

        // 1. Bad token returns 403
        {
            let mut s = TcpStream::connect(&addr).unwrap();
            s.write_all(b"GET /r/wrong_token HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut resp = String::new();
            s.read_to_string(&mut resp).unwrap();
            assert!(resp.starts_with("HTTP/1.1 403 Forbidden"));
        }

        // 2. Valid GET returns 200 HTML dropzone
        {
            let mut s = TcpStream::connect(&addr).unwrap();
            let req = format!("GET /r/{} HTTP/1.1\r\nHost: localhost\r\n\r\n", handle.token);
            s.write_all(req.as_bytes()).unwrap();
            let mut resp = String::new();
            s.read_to_string(&mut resp).unwrap();
            assert!(resp.starts_with("HTTP/1.1 200 OK"));
            assert!(resp.contains("Select Files / Photos"));
            assert!(resp.contains("Send Text to PC"));
            assert!(resp.contains("const fileCount = files.length;"));
            assert!(resp.contains("progressText.textContent = 'Sent ' + savedCount"));
        }

        // 3. Valid multipart upload twice -> hello.txt and hello (1).txt
        let boundary = "----FileUCTestBoundary123";
        let body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"../hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello world\r\n--{b}--\r\n",
            b = boundary
        );
        for expected_name in ["hello.txt", "hello (1).txt"] {
            let mut s = TcpStream::connect(&addr).unwrap();
            let req = format!(
                "POST /upload/{} HTTP/1.1\r\nHost: localhost\r\nContent-Type: multipart/form-data; boundary={}\r\nContent-Length: {}\r\n\r\n{}",
                handle.token,
                boundary,
                body.len(),
                body
            );
            s.write_all(req.as_bytes()).unwrap();
            let mut resp = String::new();
            s.read_to_string(&mut resp).unwrap();
            assert!(resp.starts_with("HTTP/1.1 200 OK"), "resp was: {resp}");
            let saved = tmp.path().join(expected_name);
            assert_eq!(fs::read_to_string(saved).unwrap(), "hello world");
        }

        // 4. Aborted multipart upload cleans up .part file
        {
            let mut s = TcpStream::connect(&addr).unwrap();
            let partial = format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"aborted.bin\"\r\nContent-Type: application/octet-stream\r\n\r\npartialbytes",
                b = boundary
            );
            let req = format!(
                "POST /upload/{} HTTP/1.1\r\nHost: localhost\r\nContent-Type: multipart/form-data; boundary={}\r\nContent-Length: 9999\r\n\r\n{}",
                handle.token,
                boundary,
                partial
            );
            s.write_all(req.as_bytes()).unwrap();
            let _ = s.shutdown(std::net::Shutdown::Both);
            drop(s);
            thread::sleep(Duration::from_millis(120));
            assert!(!tmp.path().join("aborted.bin").exists());
            let leftover_parts: Vec<_> = fs::read_dir(tmp.path())
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
                .collect();
            assert!(leftover_parts.is_empty(), "leftover .part files found");
        }

        // 5. Text > 256 KB returns 413
        {
            let mut s = TcpStream::connect(&addr).unwrap();
            let req = format!(
                "POST /text/{} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 300000\r\n\r\n",
                handle.token
            );
            s.write_all(req.as_bytes()).unwrap();
            let mut resp = String::new();
            s.read_to_string(&mut resp).unwrap();
            assert!(resp.starts_with("HTTP/1.1 413 Payload Too Large"));
        }

        handle.stop();
    }
}
