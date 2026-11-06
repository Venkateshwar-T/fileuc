use fileuc_daemon::ipc::{send_message, start_server, IpcMessage};
use fileuc_daemon::single_instance::acquire;

fn main() {
    let mutex_name = "FileUC_SingleInstance_Mutex";
    let pipe_name = "FileUC_Ipc_Pipe";

    match acquire(mutex_name) {
        Ok(_guard) => {
            println!("FileUC daemon started.");
            let _server = start_server(pipe_name, |msg| match msg {
                IpcMessage::OpenSettings => {
                    println!("Received request to open settings.");
                }
                IpcMessage::Quit => {
                    println!("Received request to quit.");
                    std::process::exit(0);
                }
            })
            .expect("Failed to start IPC server");

            // Keep main thread alive for now
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }
        Err(_) => {
            // Another instance is already running! Send IPC message and exit immediately.
            let _ = send_message(pipe_name, IpcMessage::OpenSettings);
            std::process::exit(0);
        }
    }
}
