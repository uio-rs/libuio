use std::io;

use libuio::net::TcpStream;

#[libuio::main]
async fn main() -> io::Result<()> {
    println!("Connecting to remote server.");

    let mut client = TcpStream::connect("[::1]", 9091)?.await?;

    println!(
        "Connected to remote server, local address: {}",
        client.addr()
    );

    client.send("Hello from client!".as_bytes()).await?;

    let mut buf = vec![0u8; 1024];
    let read = client.recv(buf.as_mut_slice()).await?;

    let str = String::from_utf8_lossy(&buf[..read]);
    println!("Server response: {}", str);
    Ok(())
}
