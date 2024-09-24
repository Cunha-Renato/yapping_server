use futures::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ip = String::new();

    println!("Host on: ");
    std::io::stdin().read_line(&mut ip).unwrap();

    let listener = TcpListener::bind(&ip.trim()).await?;
    println!("Yapping server is now running!");

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let ws_stream = accept_async(stream).await.expect("Error during WebSocket handshake");
            println!("Accetped connection!");

            let (_, mut read) = ws_stream.split();

            while let Some(Ok(msg)) = read.next().await {
                let received = msg.to_text().unwrap();
                println!("Received: {}", received);
            }
        });
    }
    
    Ok(())
}