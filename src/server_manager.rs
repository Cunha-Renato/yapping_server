use std::sync::Arc;

use futures::StreamExt;
use tokio::{net::TcpListener, sync::{mpsc::Sender, Mutex}};
use tokio_tungstenite::accept_async;
use yapping_core::l3gion_rust::{sllog::{error, info, warn}, StdError, UUID};
use tokio_tungstenite::tungstenite::Message as TkMessage;

use crate::{coms::Coms, mongo_db::MongoDBClient, notification_manager::{NotificationManager, NotificationManagerMessage}};

pub(crate) struct ServerManager {
    mongo_db_client: MongoDBClient,
    users_manager_sender: Sender<(UUID, NotificationManagerMessage)>,
}
impl ServerManager {
    pub(crate) async fn new() -> Result<Self, StdError> {
        let um = NotificationManager::new();
        let us = um.sender();
        um.start_recv();

        Ok(Self {
            mongo_db_client: MongoDBClient::new().await?,
            users_manager_sender: us,
        })
    }

    pub(crate) async fn run(&self) -> Result<(), StdError> {
        let listener = TcpListener::bind("0.0.0.0:8080").await?; // TODO: No idea if this is safe at all.
        info!("Yapping server is now running!");
    
        while let Ok((stream, _)) = listener.accept().await {
            let mongo_db = self.mongo_db_client.get_database();
            let users_manager_sender = self.users_manager_sender.clone();

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

                let coms = Arc::new(Mutex::new(Coms::new(
                    mongo_db, 
                    users_manager_sender,
                    write,
                )));
                let coms_read = Arc::clone(&coms);
                
                let read_taks = tokio::spawn(async move {
                    while let Some(Ok(msg)) = read.next().await {
                        match msg {
                            TkMessage::Binary(msg) => {
                                if let Err(e) = coms_read.lock().await.receive_msg(msg).await {
                                    error!("In Coms::receive_msg: {e}");
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
                
                if let Err(e) = coms.lock().await.shutdown().await {
                    error!("In Coms::shutdown: {e}");
                }
                warn!("Connection taks ended!");
            });
        };
        
        Ok(())
    }
}