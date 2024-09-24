use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut ip = String::new();

    println!("Host on: ");
    std::io::stdin().read_line(&mut ip).unwrap();

    let listener = TcpListener::bind(&ip.trim()).await?;
    println!("Yapping server is now running!");

    loop {
        let (mut socket, _) = listener.accept().await?;
        println!("Accepted connection!");

        tokio::spawn(async move {
            let mut buf = [0; 1024];

            match socket.read(&mut buf).await {
                Ok(n) if n == 0 => return, 
                Ok(n) => {
                    println!("Received: {}", String::from_utf8_lossy(&buf[..n]));
                }
                Err(e) => {
                    eprintln!("Failed to read from socket: {:?}", e);
                }
            }
        });
    }
}