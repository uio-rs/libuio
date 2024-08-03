use std::io;

use libuio::net::UdpSocket;

#[libuio::main]
async fn main() -> io::Result<()> {
    let mut socket = UdpSocket::new("[::]", 9091).expect("Failed to create UDP socket.");

    println!("Listening for UDP messages on: {:?}", socket.local_addr());

    let remote = "[::1]:9092".parse().unwrap();
    let data = String::from("Hello world!").into_bytes().to_vec();

    println!("Sending UDP message to remote: {:?}", remote);

    // We can operate in unconnected mode via send/recv_to and send/recv_msg.
    match socket.send_to(data, Some(remote)).await {
        Ok((sent, _)) => println!("Sent {} bytes to {:?}.", sent, remote),
        Err(e) => println!("Failed to send data to remote: {}", e),
    };

    // As usual we can connect to a remote address and use send/recv directly.
    socket.connect(&remote).await?;

    // We create a buffer to use for recv, and since we are using io_uring, we have to
    // pass ownership of this buffer to the ring itself, it's then returned with the correct
    // size, and with no copies involved.
    let buf = Vec::with_capacity(1024);
    match socket.recv(buf).await {
        Ok(buf) => println!(
            "Received {} bytes (cap {}) from {:?} message: {}",
            buf.len(),
            buf.capacity(),
            socket.peer_addr(),
            String::from_utf8_lossy(&buf)
        ),
        Err(e) => println!("Failed to receive data from remote: {}", e),
    };

    Ok(())
}
