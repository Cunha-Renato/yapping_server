use std::collections::HashMap;

use futures::{stream::SplitSink, SinkExt};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use yapping_core::{client_server_coms::{ComsManager, Query, Response, ServerMessage, ServerMessageContent, Session}, l3gion_rust::{sllog::{info, warn}, StdError, UUID}};
use tokio_tungstenite::tungstenite::Message as TkMessage;
use crate::mongo_db::MongoDB;

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

pub(crate) struct Coms {
    mongo_db: MongoDB,
    manager: ComsManager,
    write: SplitSink<WebSocketStream<TcpStream>, TkMessage>,
}
impl Coms {
    pub(crate) fn new(
        mongo_db: MongoDB,
        write: SplitSink<WebSocketStream<TcpStream>, TkMessage>
    ) -> Self 
    {
        Self {
            mongo_db,
            manager: ComsManager::default(),
            write,
        }
    }
    
    pub(crate) async fn update(&mut self) -> Result<(), StdError> {
        self.manager.update();
        let msgs = self.manager.to_retry();
        self.handle_msg(msgs).await?;
        
        Ok(())
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
                ServerMessageContent::NOTIFICATION(_) => todo!(),
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
    
    async fn handle_session(&self, msg_uuid: UUID, session: Session) -> ServerMessage {
        match match session {
            Session::LOGIN(info) => self.mongo_db
                .login(info).await
                .map(|user| {
                    create_response!(Response::OK_SESSION, msg_uuid, Session::TOKEN(user))
                }),
            Session::SIGN_UP(info) => self.mongo_db
                .sign_up(info).await
                .map(|user| {
                    create_response!(Response::OK_SESSION, msg_uuid, Session::TOKEN(user))
                }),
            Session::TOKEN(_) => todo!(),
        } {
            Ok(msg) => msg,
            Err(e) => create_response!(Response::Err, msg_uuid, e.to_string()),
        }
    }
    
    async fn handle_query(
        &self,
        msg_uuid: UUID,
        query: Query,
    ) -> ServerMessage {
        match match query {
            Query::USERS_BY_TAG(tags) => {
                let users = self.mongo_db.query_by_tag(tags).await;
                Ok(create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT(users)))
            },
            Query::USERS_CONTAINS_TAG(tag) => self.mongo_db
                .query_contains_tag(tag).await
                .map(|users| {
                    create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT(users))
                }),
            Query::USERS_BY_UUID(uuids) => {
                let users = self.mongo_db.query_by_uuid(uuids).await;
                Ok(create_response!(Response::OK_QUERY, msg_uuid, Query::RESULT(users)))
            },
    
            _ => Err(std::format!("Invalid Query! {:#?}", query).into()),
        } {
            Ok(msg) => msg,
            Err(e) => create_response!(Response::Err, msg_uuid, e.to_string()),
        }
    }
}

fn deserialize(bytes: Vec<u8>) -> Result<ServerMessage, StdError> {
    Ok(yapping_core::bincode::deserialize::<ServerMessage>(&bytes)?)
}

fn serialize(msg: &ServerMessage) -> Result<Vec<u8>, StdError> {
    Ok(yapping_core::bincode::serialize(msg)?)
}