use libuio::{executor::ThreadPoolBuilder, net::UdpSocket};

fn main() {
    // First we need to create a new thread pool to execute on.
    let pool = ThreadPoolBuilder::new()
        .name_prefix("executor")
        .create()
        .expect("Failed to configure thread pool.");

    // Now we spawn our main async task, which will drive any/all async operations needed by our
    // application.
    pool.spawn_ok(async {
        let mut socket = UdpSocket::new("[::]", 9091).expect("Failed to create UDP socket.");

        println!("Listening for UDP messages on: {:?}", socket.addr());

        let remote = "[::1]:9092".parse().unwrap();
        let mut data = String::from("Hello world!").into_bytes();

        println!("Sending UDP message to remote: {:?}", remote);

        match socket.send_to(data.as_mut_slice(), Some(&remote)).await {
            Ok(sent) => println!("Sent {} bytes to {:?}.", sent, remote),
            Err(e) => println!("Failed to send data to remote: {}", e),
        };

        let mut buf = vec![0u8; 1024];
        match socket.recv_from(buf.as_mut_slice()).await {
            Ok((recv, addr)) => println!(
                "Received {} bytes from {:?} message: {}",
                recv,
                addr,
                String::from_utf8_lossy(&buf[..recv])
            ),
            Err(e) => println!("Failed to receive data from remote: {}", e),
        };
    });

    pool.wait();
}
