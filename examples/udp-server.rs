use libuio::net::UdpSocket;

#[libuio::main]
async fn main() {
    let mut socket = UdpSocket::new("[::]", 9092).expect("Failed to create UDP socket.");

    println!("Listening for UDP messages on: {:?}", socket.local_addr());

    loop {
        let mut bufs = Vec::with_capacity(8);
        for _ in 0..8 {
            bufs.push(vec![0u8; 2]);
        }

        let (bufs, addr) = match socket.recv_msg(bufs).await {
            Ok((bufs, addr)) => {
                let mut raw = Vec::new();
                for buf in bufs.iter() {
                    if buf.is_empty() {
                        continue;
                    }
                    raw.extend(buf);
                }
                println!(
                    "Received {} bytes from {:?} message: {}",
                    raw.len(),
                    addr,
                    String::from_utf8_lossy(&raw)
                );
                (bufs, addr)
            }
            Err(e) => {
                println!("Failed to receive data from remote: {}", e);
                continue;
            }
        };

        match socket.send_msg(bufs, Some(addr)).await {
            Ok((sent, _)) => println!("Sent {} bytes to {:?}.", sent, addr),
            Err(e) => println!("Failed to send data to remote: {}", e),
        };
    }
}
