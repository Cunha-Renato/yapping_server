use std::sync::Arc;

use crate::{coms::Coms, mongo_db::{MongoDB, MongoDBClient}, user_manager::{UserManager, UserManagerMessage}};

use futures::{SinkExt, StreamExt, TryStreamExt};
use tokio::{net::TcpListener, sync::Mutex};
use tokio_tungstenite::accept_async;
use yapping_core::{client_server_coms::{Query, Response, ServerMessage, ServerMessageContent, Session}, l3gion_rust::{sllog::{error, info, warn}, StdError, UUID}};
use tokio_tungstenite::tungstenite::Message as TkMessage;

pub(crate) struct ServerManager {
    mongo_db_client: MongoDBClient,
    user_manager_sender: std::sync::mpsc::Sender<UserManagerMessage>,
}
impl ServerManager {
    pub(crate) async fn new() -> Result<Self, StdError> {
        let user_manager = UserManager::new();
        let user_manager_sender = user_manager.sender();

        user_manager.start_recv();

        Ok(Self {
            mongo_db_client: MongoDBClient::new().await?,
            user_manager_sender,
        })
    }

    pub(crate) async fn run(&self) -> Result<(), StdError> {
        let listener = TcpListener::bind("0.0.0.0:8080").await?; // TODO: No idea if this is safe at all.
        info!("Yapping server is now running!");
    
        while let Ok((stream, _)) = listener.accept().await {
            let mongo_db = self.mongo_db_client.get_database();
            let mut user_manager_sender = self.user_manager_sender.clone();

            tokio::spawn(async move {
                info!("New connection task spawned!");

                let ws_stream = match accept_async(stream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        error!("Handshake failed! {e}");
                        return;
                    },
                };
                info!("Accetped connection!");

                let (write, mut read) = ws_stream.split();

                let coms = Arc::new(Mutex::new(Coms::new(mongo_db, write)));
                let coms_read = Arc::clone(&coms);
                
                let read_taks = tokio::spawn(async move {
                    while let Some(Ok(msg)) = read.next().await {
                        match msg {
                            TkMessage::Binary(msg) => {
                                if let Err(e) = coms_read.lock().await.receive_msg(msg).await {
                                    error!("Faield to process ServerMessage! {e}");
                                }
                            }
                            _ => ()
                        };
                    }
                });
                
                loop {
                    if let Err(e) = coms.lock().await.update().await {
                        error!("In Coms::update()! {e}");
                    }
                    
                    if read_taks.is_finished() {
                        break;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                
                warn!("Connection taks ended!");
            });
        };
        
        Ok(())
    }
}