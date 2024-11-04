use crate::{mongo_db::{MongoDB, MongoDBClient}, user_manager::{ServerUser, UserManager, UserManagerMessage}};

use futures::{SinkExt, StreamExt, TryStreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use yapping_core::{client_server_coms::{Response, ServerMessage, ServerMessageContent, Session}, l3gion_rust::{sllog::{error, info, warn}, StdError, UUID}};
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
            let mongodb = self.mongo_db_client.get_database();
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

                let mut user = ServerUser::new();

                let (mut write, mut read) = ws_stream.split();
                while let Some(Ok(msg)) = read.next().await {
                    match msg {
                        TkMessage::Binary(client_message_bin) => if let Ok(client_message) = yapping_core::bincode::deserialize::<ServerMessage>(&client_message_bin) {
                            info!("Received: {:#?}", client_message);

                            let server_message = match client_message.content {
                                ServerMessageContent::RESPONSE(response) => todo!(),
                                ServerMessageContent::SESSION(session) => handle_session(&mongodb, client_message.uuid, session).await,
                                ServerMessageContent::NOTIFICATION(notification) => todo!(),
                                ServerMessageContent::MODIFICATION(modification) => todo!(),
                            };

                            if let Ok(server_message_bin) = yapping_core::bincode::serialize(&server_message) {
                                match write.send(TkMessage::Binary(server_message_bin)).await {
                                    Ok(_) => warn!("Sent: {:#?}", server_message),
                                    Err(e) => error!("Failed to send message! {e}"),
                                }
                            }
                        },
                        _ => (),
                    };
                }
                
                warn!("Connection taks ended!");
            });
        };
        
        Ok(())
    }
}

macro_rules! create_response {
    ($response_type:expr, $msg_uuid:expr, $content:expr) => {
        ServerMessage::new($msg_uuid, ServerMessageContent::RESPONSE($response_type($content)))
    };
}

macro_rules! create_empty_response {
    ($response_type:expr, $msg_uuid:expr) => {
        ServerMessage::new($msg_uuid, ServerMessageContent::RESPONSE($response_type(None)))
    };
}

async fn handle_session(
    mongo_db: &MongoDB,
    msg_uuid: UUID,
    session: Session,
) -> ServerMessage {
    match match session {
        Session::LOGIN(info) => mongo_db
            .login(info).await
            .map(|user| {
                create_response!(Response::OK_SESSION, msg_uuid, Session::TOKEN(user))
            }),
        Session::SIGN_UP(info) => mongo_db
            .login(info).await
            .map(|user| {
                create_response!(Response::OK_SESSION, msg_uuid, Session::TOKEN(user))
            }),
        Session::TOKEN(user) => todo!(),
    } {
        Ok(msg) => msg,
        Err(e) => create_response!(Response::Err, msg_uuid, e.to_string()),
    }
}