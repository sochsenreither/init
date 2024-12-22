use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Instant,
};

fn main() {
    let mut stream = UnixStream::connect("service_a_socket").unwrap();

    println!("Sending request to A");
    let now = Instant::now();
    stream.write_all(b"Hello").unwrap();

    let mut response = vec![0; 1024];

    let n = stream.read(&mut response).unwrap();

    println!(
        "Received \"{}\" after {:?}",
        String::from_utf8_lossy(&response[..n]),
        now.elapsed()
    );
}
