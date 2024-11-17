use std::collections::HashMap;

use tokio::sync::broadcast::{Receiver, Sender};
use yapping_core::{client_server_coms::Notification, l3gion_rust::{sllog::error, UUID}, message::Message};

#[derive(Default)]
pub(crate) struct ChatManager {
    // Chat UUID, Sender
    chats: HashMap<UUID, Sender<Notification>> 
}
impl ChatManager {
    pub(crate) fn new_chat(&mut self, chat_uuid: UUID) {
        self.chats.entry(chat_uuid).or_insert(Sender::new(50));
    }

    pub(crate) fn post(&mut self, chat_uuid: UUID, notification: Notification) {
        let sender = self.chats.entry(chat_uuid).or_insert(Sender::new(50));
        if let Err(e) = sender.send(notification) {
            error!("{e}");
        }
    }

    pub(crate) fn subscribe(&mut self, chat_uuid: UUID) -> Option<Receiver<Notification>> {
        self.chats.get(&chat_uuid)
            .map(|sender| sender.subscribe())
    }
}