use futures::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use yapping_core::l3gion_rust::{sllog::{error, info}, StdError};

pub(crate) async fn run() -> Result<(), StdError> {
    let mut ip = String::new();

    println!("Host on: ");
    std::io::stdin().read_line(&mut ip).unwrap();

    let listener = TcpListener::bind(&ip.trim()).await?;
    info!("Yapping server is now running!");

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("Handshake failed! {e}");
                    return;
                },
            };
            info!("Accetped connection!");

            let (_, mut read) = ws_stream.split();
            while let Some(Ok(msg)) = read.next().await {
                let received = msg.to_text().unwrap();
                info!("Received: ");
                println!("{received}");
            }
        });
    };
    
    Ok(())
}