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
        let mut socket = UdpSocket::new("[::]", 9092).expect("Failed to create UDP socket.");
        let mut bufs = Vec::with_capacity(8);
        for _ in 0..8 {
            bufs.push(vec![0u8; 2]);
        }

        println!("Listening for UDP messages on: {:?}", socket.addr());

        loop {
            let (recv, addr, raw) = match socket.recv_msg(bufs.as_mut_slice()).await {
                Ok((recv, addr)) => {
                    let mut raw = Vec::with_capacity(recv);
                    let mut current = recv;
                    for buf in bufs.iter() {
                        let end = if current > buf.len() {
                            buf.len()
                        } else {
                            current
                        };
                        raw.extend(buf.iter().take(end));
                        current -= end;
                        if current == 0 {
                            break;
                        }
                    }
                    println!(
                        "Received {} bytes from {:?} message: {}",
                        recv,
                        addr,
                        String::from_utf8_lossy(&raw)
                    );
                    (recv, addr, raw)
                }
                Err(e) => {
                    println!("Failed to receive data from remote: {}", e);
                    continue;
                }
            };

            let mut send_bufs = vec![
                Vec::from_iter(raw[..recv / 2].iter().copied()),
                Vec::from_iter(raw[recv / 2..].iter().copied()),
            ];
            match socket.send_msg(send_bufs.as_mut_slice(), Some(&addr)).await {
                Ok(sent) => println!("Sent {} bytes to {:?}.", sent, addr),
                Err(e) => println!("Failed to send data to remote: {}", e),
            };
        }
    });

    pool.wait();
}
