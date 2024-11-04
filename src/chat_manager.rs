use std::collections::HashMap;

use tokio::sync::broadcast::{Receiver, Sender};
use yapping_core::{l3gion_rust::UUID, client_server_coms::ServerMessage};

#[derive(Default)]
pub(crate) struct ChatManager {
    // Chat UUID, Sender
    chats: HashMap<UUID, Sender<ServerMessage>> 
}
impl ChatManager {
    pub(crate) fn subscribe(&mut self, chat_uuid: UUID) -> Option<Receiver<ServerMessage>> {
        self.chats.get(&chat_uuid)
            .map(|sender| sender.subscribe())
    }
}