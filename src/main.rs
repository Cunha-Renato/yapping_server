use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Yapping server is now running!");

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = [0; 1024];

            match socket.read(&mut buf).await {
                Ok(n) if n == 0 => return, 
                Ok(n) => {
                    println!("Received: {}", String::from_utf8_lossy(&buf[..n]));
                    // socket.write_all(&buf[0..n]).await.unwrap();
                }
                Err(e) => {
                    eprintln!("Failed to read from socket: {:?}", e);
                }
            }
        });
    }
}