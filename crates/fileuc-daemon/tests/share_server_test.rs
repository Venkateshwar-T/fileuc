use fileuc_daemon::share_server::{start_share_server, ShareServerEvent};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use tempfile::NamedTempFile;

#[test]
fn test_share_server_single_file_download() {
    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    let test_data = b"Hello, this is a test payload for FileUC local share!";
    temp.write_all(test_data).unwrap();
    temp.flush().unwrap();

    let path = temp.path().to_path_buf();
    let mut handle = start_share_server(&[path]).expect("Failed to start share server");

    assert!(handle.url.starts_with("http://"));
    assert!(handle.port > 0);
    assert_eq!(handle.file_size, test_data.len() as u64);

    // 1. Test GET /s/<token> (Mobile HTML Page)
    let mut stream = TcpStream::connect(("127.0.0.1", handle.port)).expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    let req = format!("GET /s/{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", handle.token);
    stream.write_all(req.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"), "Expected 200 OK, got: {}", status_line);

    let mut body = String::new();
    reader.read_to_string(&mut body).unwrap();
    assert!(body.contains("<!DOCTYPE html>"));
    assert!(body.contains("Download to Device"));

    // Check event received
    let event = handle.rx.recv_timeout(Duration::from_millis(500)).expect("Did not receive event");
    assert_eq!(event, ShareServerEvent::ClientConnected);

    // 2. Test GET /d/<token>/<filename> (File Binary Download)
    let mut stream2 = TcpStream::connect(("127.0.0.1", handle.port)).expect("Failed to connect 2");
    stream2.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req2 = format!(
        "GET /d/{}/{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        handle.token, handle.file_name
    );
    stream2.write_all(req2.as_bytes()).unwrap();

    let mut reader2 = BufReader::new(stream2);
    let mut status_line2 = String::new();
    reader2.read_line(&mut status_line2).unwrap();
    assert!(status_line2.contains("200 OK"));

    // Read headers until blank line
    let mut found_content_disp = false;
    let mut line = String::new();
    while let Ok(n) = reader2.read_line(&mut line) {
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if line.to_lowercase().contains("content-disposition") {
            found_content_disp = true;
        }
        line.clear();
    }
    assert!(found_content_disp, "Response must include Content-Disposition header");

    let mut downloaded_bytes = Vec::new();
    reader2.read_to_end(&mut downloaded_bytes).unwrap();
    assert_eq!(downloaded_bytes, test_data);

    // Verify download finished event
    let dl_start = handle.rx.recv_timeout(Duration::from_millis(500)).expect("Expected DownloadStarted");
    assert_eq!(dl_start, ShareServerEvent::DownloadStarted);

    let dl_finish = handle.rx.recv_timeout(Duration::from_millis(500)).expect("Expected DownloadFinished");
    assert_eq!(dl_finish, ShareServerEvent::DownloadFinished);

    // 3. Stop server cleanly
    handle.stop();
}

#[test]
fn test_share_server_multi_file_creates_zip() {
    let mut temp1 = NamedTempFile::new().unwrap();
    temp1.write_all(b"File 1 contents").unwrap();
    let mut temp2 = NamedTempFile::new().unwrap();
    temp2.write_all(b"File 2 contents").unwrap();

    let files = vec![temp1.path().to_path_buf(), temp2.path().to_path_buf()];
    let mut handle = start_share_server(&files).expect("Multi-file server start failed");

    assert_eq!(handle.file_name, "Shared_Files.zip");
    assert!(handle.file_size > 0);

    // Download the zip
    let mut stream = TcpStream::connect(("127.0.0.1", handle.port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!("GET /d/{}/Shared_Files.zip HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", handle.token);
    stream.write_all(req.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"));

    handle.stop();
}

#[test]
fn test_share_server_http_range_resume_and_head() {
    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    let test_data = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    temp.write_all(test_data).unwrap();
    temp.flush().unwrap();

    let path = temp.path().to_path_buf();
    let mut handle = start_share_server(&[path]).expect("Failed to start share server");

    // 1. Test HEAD /d/<token>/<file>
    let mut stream_head = TcpStream::connect(("127.0.0.1", handle.port)).unwrap();
    let req_head = format!(
        "HEAD /d/{}/{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        handle.token, handle.file_name
    );
    stream_head.write_all(req_head.as_bytes()).unwrap();

    let mut reader_head = BufReader::new(stream_head);
    let mut status_head = String::new();
    reader_head.read_line(&mut status_head).unwrap();
    assert!(status_head.contains("200 OK"), "HEAD must return 200 OK");

    let mut found_accept_ranges = false;
    let mut line = String::new();
    while let Ok(n) = reader_head.read_line(&mut line) {
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if line.to_lowercase().contains("accept-ranges: bytes") {
            found_accept_ranges = true;
        }
        line.clear();
    }
    assert!(found_accept_ranges, "Must contain Accept-Ranges: bytes");

    let mut body_head = Vec::new();
    reader_head.read_to_end(&mut body_head).unwrap();
    assert!(body_head.is_empty(), "HEAD response body must be empty");

    // 2. Test Range Request: Resume download from byte 10 to byte 25
    let mut stream_range = TcpStream::connect(("127.0.0.1", handle.port)).unwrap();
    let req_range = format!(
        "GET /d/{}/{} HTTP/1.1\r\nHost: localhost\r\nRange: bytes=10-25\r\nConnection: close\r\n\r\n",
        handle.token, handle.file_name
    );
    stream_range.write_all(req_range.as_bytes()).unwrap();

    let mut reader_range = BufReader::new(stream_range);
    let mut status_range = String::new();
    reader_range.read_line(&mut status_range).unwrap();
    assert!(
        status_range.contains("206 Partial Content"),
        "Range request must return 206 Partial Content, got: {}",
        status_range
    );

    let mut found_content_range = false;
    line.clear();
    while let Ok(n) = reader_range.read_line(&mut line) {
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if line.to_lowercase().contains("content-range: bytes 10-25/") {
            found_content_range = true;
        }
        line.clear();
    }
    assert!(found_content_range, "Must include valid Content-Range header");

    let mut range_body = Vec::new();
    reader_range.read_to_end(&mut range_body).unwrap();
    assert_eq!(range_body, &test_data[10..=25], "Body must match exact requested range");

    // 3. Test Range Request: Resume to end: bytes=30-
    let mut stream_resume = TcpStream::connect(("127.0.0.1", handle.port)).unwrap();
    let req_resume = format!(
        "GET /d/{}/{} HTTP/1.1\r\nHost: localhost\r\nRange: bytes=30-\r\nConnection: close\r\n\r\n",
        handle.token, handle.file_name
    );
    stream_resume.write_all(req_resume.as_bytes()).unwrap();

    let mut reader_resume = BufReader::new(stream_resume);
    let mut status_resume = String::new();
    reader_resume.read_line(&mut status_resume).unwrap();
    assert!(status_resume.contains("206 Partial Content"));

    let mut found_resume_range = false;
    line.clear();
    while let Ok(n) = reader_resume.read_line(&mut line) {
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let total = test_data.len();
        if line.to_lowercase().contains(&format!("content-range: bytes 30-{}/{}", total - 1, total)) {
            found_resume_range = true;
        }
        line.clear();
    }
    assert!(found_resume_range, "Must include Content-Range header for resume to end");

    let mut resume_body = Vec::new();
    reader_resume.read_to_end(&mut resume_body).unwrap();
    assert_eq!(resume_body, &test_data[30..]);

    handle.stop();
}
