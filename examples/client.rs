use std::{io, net::SocketAddr};

use libuio::net::TcpStream;

#[libuio::main]
async fn main() -> io::Result<()> {
    println!("Connecting to remote server.");

    let remote_addr: SocketAddr = "[::1]:9091".parse().unwrap();
    let mut client = TcpStream::new(false)?;

    // Connect to the defined remote host.
    client.connect(&remote_addr).await?;

    println!(
        "Connected to remote peer {}, local address: {}",
        client.peer_addr(),
        client.local_addr(),
    );

    // Send some data to the remote host.
    client
        .send("Hello from client!".as_bytes().to_vec())
        .await?;

    // Now read back anything the server sent and then exit.
    let buf = client.recv(Vec::with_capacity(1024)).await?;

    let str = String::from_utf8_lossy(&buf);
    println!("Server response: {}", str);
    Ok(())
}
