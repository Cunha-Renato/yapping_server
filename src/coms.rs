use futures::{stream::SplitSink, SinkExt};
use tokio::{net::TcpStream, sync::mpsc::{Receiver, Sender}};
use tokio_tungstenite::WebSocketStream;
use yapping_core::{client_server_coms::{ComsManager, Notification, NotificationType, Query, Response, ServerMessage, ServerMessageContent, Session}, l3gion_rust::{sllog::{error, info, warn}, StdError, UUID}};
use tokio_tungstenite::tungstenite::Message as TkMessage;
use crate::{mongo_db::MongoDB, notification_manager::NotificationManagerMessage};

macro_rules! create_response {
    ($response_type:expr, $msg_uuid:expr, $content:expr) => {
        ServerMessage::new($msg_uuid, ServerMessageContent::RESPONSE($response_type($content)))
    };
}

pub(crate) struct Coms {
    user_uuid: UUID,
    notification_sender: Sender<Notification>,
    notification_receiver: Receiver<Notification>,
    
    notification_manager_sender: Sender<(UUID, NotificationManagerMessage)>,

    mongo_db: MongoDB,
    manager: ComsManager,
    write: SplitSink<WebSocketStream<TcpStream>, TkMessage>,
}
impl Coms {
    pub(crate) fn new(
        mongo_db: MongoDB,
        notification_manager_sender: Sender<(UUID, NotificationManagerMessage)>,
        write: SplitSink<WebSocketStream<TcpStream>, TkMessage>,
    ) -> Self 
    {
        let (notification_sender, notification_receiver) = tokio::sync::mpsc::channel(100);

        Self {
            user_uuid: UUID::default(),
            notification_sender,
            notification_receiver,
            
            notification_manager_sender,

            mongo_db,
            manager: ComsManager::default(),
            write,
        }
    }
    
    pub(crate) async fn update(&mut self) -> Result<(), StdError> {
        self.manager.update();

        for msg in self.manager.to_retry() {
            self.send_msg(Some(msg)).await?;
        }
    
        while let Ok(notification) = self.notification_receiver.try_recv() {
            match &notification.notification_type {
                NotificationType::FRIEND_ACCEPTED(_, _) => self.re_send_user().await?,
                _ => self.send_msg(Some(ServerMessage::from(ServerMessageContent::NOTIFICATION(notification)))).await?,

            };
        }
        
        Ok(())
    }

    pub(crate) async fn shutdown(&self) -> Result<(), tokio::sync::mpsc::error::SendError<(UUID, NotificationManagerMessage)>> {
        self.notification_manager_sender.send((
            self.user_uuid,
            NotificationManagerMessage::USER_OFFLINE
        ))
        .await
    }

    pub(crate) async fn receive_msg(&mut self, msg: Vec<u8>) -> Result<(), StdError> {
        let msg = deserialize(msg)?;
        info!("Received Message: {:#?}", msg);
        self.manager.received(msg);
        let msgs = self.manager.received_waiting();
        self.handle_msg(msgs).await?;

        Ok(())
    }
}
// Private
impl Coms {
    async fn handle_msg(&mut self, msgs: Vec<ServerMessage>) -> Result<(), StdError> {
        for msg in msgs {
            let response_msg = match msg.content {
                ServerMessageContent::SESSION(session) => Some(self.handle_session(msg.uuid, session).await),
                ServerMessageContent::NOTIFICATION(notification) => Some(self.handle_notification(msg.uuid, notification).await?),
                ServerMessageContent::MODIFICATION(_) => todo!(),
                ServerMessageContent::QUERY(query) => Some(self.handle_query(msg.uuid, query).await),
                _ => None,
            };
            
            self.send_msg(response_msg).await?;
        }
        
        Ok(())
    }
    
    async fn send_msg(&mut self, msg: Option<ServerMessage>) -> Result<(), StdError> {
        let msg = msg.ok_or("Message received is a Response!")?;
        let bin_msg = TkMessage::Binary(serialize(&msg)?);
        self.write.send(bin_msg).await?;

        warn!("Sent: {:#?}", msg);

        self.manager.sent(msg);

        Ok(())
    }
    
    async fn handle_session(&mut self, msg_uuid: UUID, session: Session) -> ServerMessage {
        let (user_uuid, msg) = match match session {
            Session::LOGIN(info) => self.mongo_db
                .login(info).await
                .map(|user| {
                    (user.uuid(), create_response!(Response::OK_SESSION, msg_uuid, Session::TOKEN(user)))
                }),
            Session::SIGN_UP(info) => self.mongo_db
                .sign_up(info).await
                .map(|user| {
                    (user.uuid(), create_response!(Response::OK_SESSION, msg_uuid, Session::TOKEN(user)))
                }),
            Session::TOKEN(_) => todo!(),
        } {
            Ok(content) => content,
            Err(e) => return create_response!(Response::Err, msg_uuid, e.to_string()),
        };
        
        self.user_uuid = user_uuid;
        if let Err(e) = self.notification_manager_sender.send((
            self.user_uuid,
            NotificationManagerMessage::NOTIFY_USER(self.user_uuid, self.notification_sender.clone())
        )).await {
            error!("In Coms::handle_session: {e}");
        }

        msg
    }
    
    async fn handle_query(
        &self,
        msg_uuid: UUID,
        query: Query,
    ) -> ServerMessage {
        if !self.is_user_valid() { 
            return ServerMessage::new(msg_uuid, ServerMessageContent::RESPONSE(Response::Err("User is not logged in, Server can't respond to query!".to_string()))); 
        }

        match match query {
            Query::USERS_BY_TAG(tags) => {
                let users = self.mongo_db.query_by_tag(tags).await;
                Ok(create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT_USER(users)))
            },
            Query::USERS_CONTAINS_TAG(tag) => self.mongo_db
                .query_contains_tag(tag).await
                .map(|users| create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT_USER(users))),
            Query::USERS_BY_UUID(uuids) => {
                let users = self.mongo_db.query_by_uuid(uuids).await;
                Ok(create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT_USER(users)))
            },
            Query::FRIEND_REQUESTS => {
                self.mongo_db.get_user_friend_requests(self.user_uuid).await
                    .map(|notifications| create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT_FRIEND_REQUESTS(notifications)))
            }
    
            _ => Err(std::format!("Invalid Query! {:#?}", query).into()),
        } {
            Ok(msg) => msg,
            Err(e) => create_response!(Response::Err, msg_uuid, e.to_string()),
        }
    }
    
    async fn handle_notification(&mut self, msg_uuid: UUID, notification: Notification) -> Result<ServerMessage, StdError> {
        /*
            ADD notification to the users database,
            SEND notification to users,
            HANDLE response from users,
        */

        if !self.is_user_valid() { return Err("User is not logged in, Server can't respond to notifications!".into()); }

        // Handling the database
        match notification.notification_type {
            NotificationType::FRIEND_REQUEST(sender, receiver) => {
                if self.user_uuid == sender {
                    // Saving the notification in the database.
                    self.mongo_db.insert_notification(receiver, &notification).await?;
                }
            },
            NotificationType::FRIEND_ACCEPTED(sender, receiver) => {
                if self.user_uuid == sender {
                    // Removing the notification in the database.
                    // Add the friend for both users

                    self.mongo_db.remove_notification(notification.uuid()).await?;
                    self.mongo_db.insert_friend(self.user_uuid, receiver).await?;
                    self.mongo_db.insert_friend(receiver, self.user_uuid).await?;
                        
                    self.re_send_user().await?;
                }

            },
            _ => (),
        };

        if let Err(e) = self.notification_manager_sender.send((
            self.user_uuid,
            NotificationManagerMessage::CLIENT_MESSAGE(notification)
        )).await {
            error!("In Coms::handle_notification: {e}");
        }
        
        Ok(ServerMessage::new(msg_uuid, ServerMessageContent::RESPONSE(Response::OK)))
    }

    async fn re_send_user(&mut self) -> Result<(), StdError> {
        let user = self.mongo_db.get_full_user(self.user_uuid).await?;
        self.send_msg(Some(ServerMessage::from(ServerMessageContent::SESSION(Session::TOKEN(user))))).await
    }

    fn is_user_valid(&self) -> bool {
        self.user_uuid.get_value() != 0
    }
}

fn deserialize(bytes: Vec<u8>) -> Result<ServerMessage, StdError> {
    Ok(yapping_core::bincode::deserialize::<ServerMessage>(&bytes)?)
}

fn serialize(msg: &ServerMessage) -> Result<Vec<u8>, StdError> {
    Ok(yapping_core::bincode::serialize(msg)?)
}