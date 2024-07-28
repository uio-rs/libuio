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
    client.send("Hello from client!".as_bytes()).await?;

    // Now read back anything the server sent and then exit.
    let mut buf = vec![0u8; 1024];
    let read = client.recv(buf.as_mut_slice()).await?;

    let str = String::from_utf8_lossy(&buf[..read]);
    println!("Server response: {}", str);
    Ok(())
}
