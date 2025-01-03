use std::{
    os::{fd::FromRawFd, unix::net::UnixListener},
    time::Duration,
};

use init::Worker;
use tokio::time::sleep;

// We don't manually bind, instead we just receive the fd from init.
const _SOCKET: &'static str = "service_b_socket";

#[tokio::main]
async fn main() {
    setup().await;

    // We use socket activation, so lets receive the fd from init!
    // let listener = UnixListener::bind(_SOCKET).unwrap();
    let listener = unsafe { UnixListener::from_raw_fd(init::init_get_fd().unwrap()) };

    loop {
        let (stream, _) = listener.accept().unwrap();
        let mut worker = Worker::new("B", stream);
        tokio::spawn(async move {
            worker.run(|| {});
        });
    }
}

async fn setup() {
    env_logger::init();
    log::info!("Starting");

    sleep(Duration::from_secs(2)).await;

    // This service needs data from serviceA for its setup. Will fail if serviceA is not up.
    log::trace!("B needs data from C for setup");
    init::request("service_c_socket");
}
