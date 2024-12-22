use std::{
    os::{fd::FromRawFd, unix::net::UnixListener},
    time::Duration,
};

use init::Worker;
use tokio::time::sleep;

// We don't manually bind, instead we just receive the fd from init.
const _SOCKET: &'static str = "service_a_socket";

#[tokio::main]
async fn main() {
    setup().await;

    // We use socket activation, so lets receive the fd from init!
    // let listener = UnixListener::bind(_SOCKET).unwrap();
    let listener = unsafe { UnixListener::from_raw_fd(init::init_get_fd().unwrap()) };

    loop {
        let (stream, _address) = listener.accept().unwrap();
        let mut worker = Worker::new("A", stream);
        // Spawn a worker that receives requests, calls a function and then sends an answer
        tokio::spawn(async move {
            worker.run(request_b);
        });
    }
}

/// Requests an answer from service B.
fn request_b() {
    init::request("service_b_socket");
}

async fn setup() {
    env_logger::init();
    log::info!("Starting");

    sleep(Duration::from_secs(3)).await;
}
