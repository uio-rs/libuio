use std::io;

use futures::StreamExt;

use libuio::net::TcpListener;

#[libuio::main]
async fn main() -> io::Result<()> {
    // Since we are demonstrating a TCP server, lets start by creating a new TcpListener that
    // is set to listen on [::]:9091 and have a connection backlog of 1024.
    let mut listener = TcpListener::with_outstanding("[::]", 9091, 1024)?;

    let mut buf = vec![0u8; 1024];

    println!("Listening on: {}", listener.addr());

    // Or we can grab a async stream of incoming connections, this is using the
    // opcode::AcceptMulti, which is a highly efficient implementation of the standard accept
    // loop. This will loop endlessly until dropped or there is an unrecoverable error.
    //
    // Note that you want to call incoming OUTSIDE of a loop like bellow, otherwise you will
    // be implicitly droping/recrating the incoming future which results in performance worse
    // than that of a a `listener.accept().await` loop would provide.
    let mut incoming = listener.incoming();
    while let Some(conn) = incoming.next().await {
        let mut conn = match conn {
            Ok(conn) => conn,
            Err(e) => {
                println!("Oh no we had an error: {}", e);
                continue;
            }
        };

        println!("Got connection from: {}", conn.addr());

        let read = match conn.recv(buf.as_mut_slice()).await {
            Ok(ret) => ret,
            Err(e) => {
                println!("Failed to receive from client: {}", e);
                continue;
            }
        };

        let s = String::from_utf8_lossy(&buf[..read]);

        println!("Client request: {}", s);

        conn.send(&buf[..read])
            .await
            .expect("Failed to respond to client.");
    }
    Ok(())
}
