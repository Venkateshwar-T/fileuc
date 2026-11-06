use fileuc_daemon::single_instance::acquire;
use fileuc_daemon::ipc::{send_message, start_server, IpcMessage};
use std::sync::mpsc;

#[test]
fn test_single_instance_rejects_second() {
    let name = "test_fileuc_mutex_01";
    let _guard = acquire(name).expect("first succeeds");
    assert!(acquire(name).is_err(), "second must fail");
}

#[test]
fn test_ipc_message_roundtrip() {
    let (tx, rx) = mpsc::channel();
    let pipe = "test_fileuc_pipe_01";
    let _server = start_server(pipe, move |msg| { tx.send(msg).unwrap(); }).unwrap();

    send_message(pipe, IpcMessage::OpenSettings).unwrap();
    let received = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    assert_eq!(received, IpcMessage::OpenSettings);
}
