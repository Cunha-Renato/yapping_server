use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use tokio::{net::TcpListener, sync::Mutex};
use tokio_tungstenite::accept_async;
use yapping_core::{l3gion_rust::{sllog::{error, info, warn}, StdError}, server_message::{ClientMessage, ServerMessage, SuccessType}};
use crate::mongo_db::MongoDB;

use tokio_tungstenite::tungstenite::Message as TkMessage;

pub(crate) struct ServerManager {
    mongo_db: Arc<Mutex<MongoDB>>,
}
impl ServerManager {
    pub(crate) fn new(mongo_db: MongoDB) -> Self {
        Self { mongo_db: Arc::new(Mutex::new(mongo_db)) }
    }

    pub(crate) async fn run(&self) -> Result<(), StdError> {
        let mut ip = String::new();
    
        println!("Host on: ");
        std::io::stdin().read_line(&mut ip).unwrap();
    
        let listener = TcpListener::bind(&ip.trim()).await?;
        info!("Yapping server is now running!");
    
        while let Ok((stream, _)) = listener.accept().await {
            let mongodb = Arc::clone(&self.mongo_db);
            tokio::spawn(async move {
                let ws_stream = match accept_async(stream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("Handshake failed! {e}");
                        return;
                    },
                };
                info!("Accetped connection!");
    
                let (mut write, mut read) = ws_stream.split();
                while let Some(Ok(msg)) = read.next().await {
                    match msg {
                        TkMessage::Binary(client_message_bin) => if let Ok(client_message) = yapping_core::bincode::deserialize::<ClientMessage>(&client_message_bin) {
                            info!("Received: {:?}", client_message);
                            
                            let server_message = Self::handle_message(Arc::clone(&mongodb), client_message).await;
                            if let Ok(server_message_bin) = yapping_core::bincode::serialize(&server_message) {
                                match write.send(TkMessage::Binary(server_message_bin)).await {
                                    Ok(_) => warn!("Sent: {:?}", server_message),
                                    Err(e) => error!("Failed to send message! {e}"),
                                }
                            }
                        },
                        _ => (),
                    };
                }
            });
        };
        
        Ok(())
    }
}
// Private
impl ServerManager {
    async fn handle_message(mongo_db: Arc<Mutex<MongoDB>>, message: ClientMessage) -> ServerMessage {
        let mongo_db = mongo_db.lock().await;

        match match message {
            ClientMessage::LOGIN(info) => mongo_db.login(info).await.map(|user| ServerMessage::SUCCESS(SuccessType::LOGIN(user))),
            ClientMessage::SIGN_UP(info) => mongo_db.sign_up(info).await.map(|user| ServerMessage::SUCCESS(SuccessType::SIGN_UP(user))),
            ClientMessage::NEW_CHAT(_) => todo!(),
            ClientMessage::MESSAGE_SEND(_, _) => todo!(),
            ClientMessage::UPDATE_USER_TAG(_, _) => todo!(),
            ClientMessage::UPDATE_USER_EMAIL(_, _) => todo!(),
            ClientMessage::UPDATE_USER_PIC(_, _) => todo!(),
            ClientMessage::UPDATE_USER_PASSWORD(_, _) => todo!(),
            ClientMessage::DELETE_USER(_) => todo!(),
            ClientMessage::FRIEND_REQUEST(_, _) => todo!(),
        } {
            Ok(sm) => sm,
            Err(e) => ServerMessage::FAIL(e.to_string()),
        }
    }
}