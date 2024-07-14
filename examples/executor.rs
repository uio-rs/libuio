use std::os::fd::AsRawFd;

use futures::StreamExt;

use libuio::{executor::ThreadPoolBuilder, net::TcpListener};

fn main() {
    // First we need to create a new thread pool to execute on.
    let pool = ThreadPoolBuilder::new()
        .name_prefix("executor")
        .create()
        .expect("Failed to configure thread pool.");

    // Now we spawn our main async task, which will drive any/all async operations needed by our
    // application.
    pool.spawn_ok(async {
        // Since we are demonstrating a TCP server, lets start by creating a new TcpListener that
        // is set to listen on [::]:9091 and have a connection backlog of 1024.
        let mut listener = TcpListener::with_outstanding("[::]", 9091, 1024)
            .expect("Failed to configure listener.");

        println!("Accepting a single connection.");

        let buf = "Hello world!".as_bytes();

        // We can then call accept() to capture a single connection, this is using the
        // opcode::Accept and loosely matches the semantics of accept(4).
        match listener.accept().await {
            Ok(mut conn) => {
                println!(
                    "Got a new connection on my fancy new ACCEPT setup: {}",
                    conn.as_raw_fd()
                );
                match conn.send(buf).await {
                    Ok(n) => println!("Sent {} bytes to new connection!", n),
                    Err(e) => println!("Failed to write data to client: {}", e),
                }
            }
            Err(e) => println!("Oh no we had an error: {}", e),
        };

        println!("Setting up stream of incoming connections");

        // Or we can grab a async stream of incoming connections, this is using the
        // opcode::AcceptMulti, which is a highly efficient implementation of the standard accept
        // loop. This will loop endlessly until dropped or there is an unrecoverable error.
        //
        // Note that you want to call incoming OUTSIDE of a loop like bellow, otherwise you will
        // be implicitly droping/recrating the incoming future which results in performance worse
        // than that of a a `listener.accept().await` loop would provide.
        let mut incoming = listener.incoming();
        while let Some(conn) = incoming.next().await {
            match conn {
                Ok(conn) => println!(
                    "Got a new connection on my fancy new INCOMING setup: {}",
                    conn.as_raw_fd()
                ),
                Err(e) => println!("Oh no we had an error: {}", e),
            }
        }
    });

    pool.wait();
}
