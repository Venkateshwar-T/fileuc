use fileuc_daemon::pinggy::PinggyTunnel;
use std::time::Duration;

#[test]
fn test_pinggy_tunnel_live() {
    let tunnel = PinggyTunnel::start(8080, "/s/test1234").expect("Failed to start tunnel");
    println!("Waiting for URL...");
    match tunnel.rx_url.recv_timeout(Duration::from_secs(12)) {
        Ok(res) => {
            println!("RECEIVED RESULT: {:?}", res);
            assert!(res.is_ok(), "Expected OK url, got error: {:?}", res);
        }
        Err(e) => {
            panic!("Timed out waiting for pinggy URL: {:?}", e);
        }
    }
}
