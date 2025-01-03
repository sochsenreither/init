use std::{
    collections::BTreeMap,
    env,
    ffi::CString,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    ptr,
    sync::RwLock,
};

use init::INIT_ENV_FORMAT;
use mio::{net::UnixListener, Events, Interest, Poll, Token};

const LISTENER: Token = Token(0);

// Service name -> socket
type ServiceMap = BTreeMap<&'static str, &'static str>;

// Initialized only once. We can't register services at runtime, which we probably don't want anyway.
static SERVICE_MAP: RwLock<ServiceMap> = RwLock::new(ServiceMap::new());

#[tokio::main]
async fn main() {
    env_logger::init();
    parse();

    // Open all file descriptors.
    for (service, socket) in SERVICE_MAP.read().unwrap().iter() {
        // socket_listener will call service_spawner, which might eventually call spawn_service, which write locks
        // SERVICE_MAP. This is not a problem, since before dropping this read lock, no other process might do a
        // service request, which will trigger spawn_service, since no other process can't run at this point.
        socket_listener(service, socket).await;
    }

    log::info!("init done creating socket listeners");

    // At this point we registered all file descriptors and didn't start any service yet. So nothing will happen at
    // all.

    // Since these services all use socket activation, we can just start them in parallel and don't have to worry
    // about dependencies. Or not start them at all, so they are started on-demand.
    // start_service("serviceA");
    // start_service("serviceB");
    // start_service("serviceC");

    // Note that the information from waitpid allows us to restart services (e.g., by notifying the async task
    // responsible for starting that service).
    loop {
        let dead_child = unsafe { libc::waitpid(-1, ptr::null_mut(), 0) };
        if dead_child == -1 {
            continue;
        }
        log::info!("Child {} died", dead_child);
    }
}

/// In practice this would parse some init.rc or some config files to retrieve the sockets of services using socket
/// activation.
fn parse() {
    let mut service_map = SERVICE_MAP.write().unwrap();
    service_map.insert("serviceA", "service_a_socket");
    service_map.insert("serviceB", "service_b_socket");
    service_map.insert("serviceC", "service_c_socket");
}

/// Spawns a service by connecting to its socket.
fn start_service(service: &'static str) {
    let service_map = SERVICE_MAP.read().unwrap();
    let socket = service_map.get(service).unwrap();
    let _stream = UnixStream::connect(socket).unwrap();
}

/// Creates and listens to a Unix socket.
///
/// Once a connections comes in, a service is started that will handle the connection.
/// The created Listener is moved into the async task and will be dropped, once that task returns. This will be after
/// forking the service, so dropping is ok.
///
/// Note that this assumes services never die. Once we spawned the service we just return.
/// If we want dynamic restarting this listener needs to continously listen to the socket in order to be able to
/// restart services.
async fn socket_listener(service: &'static str, socket: &'static str) {
    let mut listener = UnixListener::bind(socket).unwrap();
    log::info!("Listening to {socket} (service: {service})");

    tokio::spawn(async move {
        service_spawner(service, &mut listener).await;
    });
}

/// Spawns a service when there is some incoming connection to the Listener.
///
/// Polls the Listener file descriptor, checking for possible events. If there is such event, a service is spawned that
/// can then accept the incoming connection.
///
/// After the service is spawned we return. If we implement automatic restarting of services, this should not return,
/// since returning also drops the Listener.
async fn service_spawner(service: &'static str, listener: &mut UnixListener) {
    let mut poll = Poll::new().unwrap();
    poll.registry()
        .register(listener, LISTENER, Interest::READABLE)
        .unwrap();

    let mut events = Events::with_capacity(128);

    poll.poll(&mut events, None).unwrap();

    for event in events.iter() {
        match event.token() {
            LISTENER => {
                log::info!("Incoming connection for {}", service);
                spawn_service(service, listener.as_raw_fd());
            }
            _ => unreachable!(),
        }
    }
}

/// Spawns a service.
///
/// This is done with a combination of fork and exec. Should probably be done with posix_spawn.
///
/// Unsets FD_CLOEXEC for the file descriptor we want to pass and makes it blocking (so for the service it looks like
/// a normal blocking UnixListener).
fn spawn_service(service: &'static str, socket: RawFd) {
    match unsafe { libc::fork() } {
        -1 => panic!("fork failed"),
        0 => {
            // File descriptors in Rust are per default FD_CLOEXEC. Lets remove that flag for our socket, so it
            // survives exec.
            unset_cloexec(socket);
            // Set file descriptor to blocking, so it appears like a UnixListener from std for services.
            unset_nonblocking(socket);
            exec(service, socket)
        }
        _pid => {
            // The parent is done here.

            // Note: we could keep a map of pid -> service, so we can automatically restart exited services.
            return;
        }
    }
}

/// Unsets FD_CLOEXEC from a given raw file descriptor.
fn unset_cloexec(fd: RawFd) {
    // Get current fd flags.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert_ne!(flags, -1);

    // Unset FD_CLOEXEC.
    let new_flags = flags & !libc::FD_CLOEXEC;
    assert_ne!(unsafe { libc::fcntl(fd, libc::F_SETFD, new_flags) }, -1);
}

/// Set the file descriptor to blocking.
fn unset_nonblocking(fd: RawFd) {
    let mut nonblocking = false as libc::c_int;
    unsafe { libc::ioctl(fd, libc::FIONBIO, &mut nonblocking) };
}

/// Executes a service.
///
/// Sets up the correct service path and arguments for the service. The file descriptor to be passed will be set as
/// environment variable.
///
/// Does an execve system call.
///
/// Panics if exec fails, since how would we even recover from that?
fn exec(service: &'static str, socket: RawFd) -> ! {
    let program_path = "target/debug/".to_string() + service;
    let program = CString::new(program_path).unwrap();

    // We start without any arguments, so we just use the program name as first argument.
    let argv = vec![program.as_ptr(), ptr::null()];

    // Set an environment variable for the passed file descriptor.
    unsafe {
        env::set_var(INIT_ENV_FORMAT, format!("{}", socket));
    }

    unsafe { libc::execvp(program.as_ptr(), argv.as_ptr()) };

    unreachable!("execve failed");
}
