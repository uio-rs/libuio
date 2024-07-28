use libuio::net::UdpSocket;

#[libuio::main]
async fn main() {
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
}
