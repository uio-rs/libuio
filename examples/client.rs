use libuio::{executor::ThreadPoolBuilder, net::TcpStream};

fn main() {
    // First we need to create a new thread pool to execute on.
    let pool = ThreadPoolBuilder::new()
        .name_prefix("executor")
        .create()
        .expect("Failed to configure thread pool.");

    // Now we spawn our main async task, which will drive any/all async operations needed by our
    // application.
    pool.spawn_ok(async {
        let mut client = TcpStream::connect("[::1]", 9091)
            .expect("Failed to create client socket")
            .await
            .expect("Failed to connect to remote server.");

        println!(
            "Connected to remote server, local address: {}",
            client.addr()
        );

        client
            .send("Hello from client!".as_bytes())
            .await
            .expect("Failed to send data to remote server");

        let mut buf = vec![0u8; 1024];
        let read = client
            .recv(buf.as_mut_slice())
            .await
            .expect("Failed to read data from remote server.");

        let str = String::from_utf8_lossy(&buf[..read]);
        println!("Server response: {}", str);
    });

    pool.wait();
}
