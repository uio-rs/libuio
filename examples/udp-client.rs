use std::io;

use libuio::net::UdpSocket;

#[libuio::main]
async fn main() -> io::Result<()> {
    let mut socket = UdpSocket::new("[::]", 9091).expect("Failed to create UDP socket.");

    println!("Listening for UDP messages on: {:?}", socket.local_addr());

    let remote = "[::1]:9092".parse().unwrap();
    let mut data = String::from("Hello world!").into_bytes();

    println!("Sending UDP message to remote: {:?}", remote);

    // We can operate in unconnected mode via send/recv_to and send/recv_msg.
    match socket.send_to(data.as_mut_slice(), Some(&remote)).await {
        Ok(sent) => println!("Sent {} bytes to {:?}.", sent, remote),
        Err(e) => println!("Failed to send data to remote: {}", e),
    };

    // As usual we can connect to a remote address and use send/recv directly.
    socket.connect(&remote).await?;

    let mut buf = vec![0u8; 1024];
    match socket.recv(buf.as_mut_slice()).await {
        Ok(recv) => println!(
            "Received {} bytes from {:?} message: {}",
            recv,
            socket.peer_addr(),
            String::from_utf8_lossy(&buf[..recv])
        ),
        Err(e) => println!("Failed to receive data from remote: {}", e),
    };

    Ok(())
}
