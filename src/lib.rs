use std::{
    env,
    io::{Read, Write},
    os::unix::net::UnixStream,
};

// Do we really want to expose this into a library used by the user?
pub const INIT_ENV_FORMAT: &'static str = "INIT_FD";

pub struct Error();

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Error").finish()
    }
}

/// Returns the raw file descriptor that was inherited by init.
///
/// Expects that an environment variable is set that indicates the file descriptor number.
/// Unsets environment variable after reading it.
///
/// This will fail, if the service was not spawned by init.
///
/// # Examples
///
/// ```rust
/// use init::init_get_fd;
///
/// let Ok(socket_fd) = init_get_fd() else {
///     // handle error
/// }
/// let socket = unsafe { UnixStream::from_raw_fd(socket_fd) };
/// socket.write_all(b"Hello, world!").unwrap();
/// ```
pub fn init_get_fd() -> Result<i32, Error> {
    match env::var(INIT_ENV_FORMAT) {
        Ok(value) => value.parse::<i32>().or(Err(Error())),
        Err(_err) => Err(Error()),
    }
}

pub struct Worker {
    service: &'static str,
    stream: UnixStream,
}

impl Worker {
    pub fn new(service: &'static str, stream: UnixStream) -> Worker {
        Worker { service, stream }
    }

    pub fn run<F>(&mut self, f: F)
    where
        F: Fn() -> (),
    {
        let mut buf = vec![0; 1024];
        loop {
            match self.stream.read(&mut buf) {
                Ok(n) => {
                    if n == 0 {
                        return;
                    }

                    log::trace!(
                        "{} got message {}",
                        self.service,
                        String::from_utf8_lossy(&buf[..n])
                    );

                    f();

                    if self
                        .stream
                        .write(format!("Answer from {}", self.service).as_bytes())
                        .is_err()
                    {
                        log::info!("Couldn't write to stream. Exiting worker");
                        return;
                    }
                }
                Err(e) => {
                    log::info!("Error while reading from socket: {}. Exiting worker", e);
                    return;
                }
            }
        }
    }
}

pub fn request(socket: &'static str) {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream.write_all(b"Asking for data").unwrap();
    let mut buf = vec![0; 1024];
    match stream.read(&mut buf) {
        Ok(n) => {
            log::trace!("Got message: {}", String::from_utf8_lossy(&buf[..n]))
        }
        Err(e) => {
            log::info!("Error while reading from socket: {}", e);
            return;
        }
    }
}
