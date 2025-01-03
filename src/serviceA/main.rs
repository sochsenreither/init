use std::{
    os::{fd::FromRawFd, unix::net::UnixListener},
    time::Duration,
};

use init::Worker;
use tokio::time::sleep;

// We don't manually bind, instead we just receive the fd from init.
const SOCKET: &'static str = "service_a_socket";

#[tokio::main]
async fn main() {
    setup().await;

    // Works with and without socket activation.
    let listener = match init::init_get_fd() {
        Ok(raw_fd) => unsafe { UnixListener::from_raw_fd(raw_fd) },
        Err(_) => UnixListener::bind(SOCKET).unwrap(),
    };

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
    log::trace!("A needs data from B");
    init::request("service_b_socket");
}

async fn setup() {
    env_logger::init();
    log::info!("Starting");

    sleep(Duration::from_secs(3)).await;
}
