use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_NONE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

pub use fileuc_types::IpcMessage;

#[derive(Debug)]
pub enum IpcError {
    WindowsError(windows::core::Error),
    SerializationError(String),
    ConnectionFailed,
    IoError(String),
}

pub struct IpcServer {
    pipe_name: String,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = send_message(&self.pipe_name, IpcMessage::Quit);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

pub fn start_server(
    pipe_name: &str,
    handler: impl Fn(IpcMessage) + Send + 'static,
) -> Result<IpcServer, IpcError> {
    let full_pipe_name = format!(r"\\.\pipe\{}", pipe_name);
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let thread_handle = thread::spawn(move || {
        let wide_name = HSTRING::from(full_pipe_name.as_str());
        while running_clone.load(Ordering::SeqCst) {
            unsafe {
                let handle = CreateNamedPipeW(
                    PCWSTR::from_raw(wide_name.as_ptr()),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    1024,
                    1024,
                    0,
                    None,
                );

                if handle == INVALID_HANDLE_VALUE {
                    thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }

                let connected = ConnectNamedPipe(handle, None);
                let is_connected = match connected {
                    Ok(_) => true,
                    Err(e) if e.code() == windows::Win32::Foundation::WIN32_ERROR(535).to_hresult() => true,
                    Err(_) => false,
                };
                if is_connected {
                    let mut buffer = [0u8; 1024];
                    let mut bytes_read = 0u32;
                    if ReadFile(
                        handle,
                        Some(&mut buffer),
                        Some(&mut bytes_read),
                        None,
                    )
                    .is_ok()
                        && bytes_read > 0
                    {
                        if let Ok(msg) =
                            serde_json::from_slice::<IpcMessage>(&buffer[..bytes_read as usize])
                        {
                            handler(msg);
                        }
                    }
                    let _ = DisconnectNamedPipe(handle);
                }
                let _ = CloseHandle(handle);
            }
        }
    });

    // Give server thread a few milliseconds to create the pipe instance
    thread::sleep(std::time::Duration::from_millis(50));

    Ok(IpcServer {
        pipe_name: pipe_name.to_string(),
        running,
        thread_handle: Some(thread_handle),
    })
}

pub fn send_message(pipe_name: &str, msg: IpcMessage) -> Result<(), IpcError> {
    let full_pipe_name = format!(r"\\.\pipe\{}", pipe_name);
    let wide_name = HSTRING::from(full_pipe_name.as_str());
    let payload = serde_json::to_vec(&msg).map_err(|e| IpcError::SerializationError(e.to_string()))?;

    unsafe {
        let handle = CreateFileW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .map_err(IpcError::WindowsError)?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(IpcError::ConnectionFailed);
        }

        let mut bytes_written = 0u32;
        let res = WriteFile(
            handle,
            Some(&payload),
            Some(&mut bytes_written),
            None,
        );

        let _ = CloseHandle(handle);
        res.map_err(IpcError::WindowsError)?;
    }

    Ok(())
}
