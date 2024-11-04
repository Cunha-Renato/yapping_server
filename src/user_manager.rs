use std::{collections::HashMap, sync::{mpsc::{Receiver, Sender}, Arc}};

use futures::lock::Mutex;
use yapping_core::{client_server_coms::{Notification, ServerMessage}, l3gion_rust::{sllog::info, UUID}, user::User};

use crate::chat_manager::ChatManager;

#[allow(non_camel_case_types)]
pub(crate) enum UserManagerMessage {
    NOTIFY_USER(UUID, Arc<Mutex<ServerUser>>),
    CLIENT_MESSAGE(Notification),
}

pub(crate) struct UserManager {
    sender: Sender<UserManagerMessage>,
    receiver: Receiver<UserManagerMessage>,
    
    users: HashMap<UUID, Arc<Mutex<ServerUser>>>,
    chat_manager: ChatManager,
}
impl UserManager {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();

        Self {
            sender,
            receiver,
            users: HashMap::default(),
            chat_manager: ChatManager::default(),
        }
    }
    
    pub(crate) fn sender(&self) -> std::sync::mpsc::Sender<UserManagerMessage> {
        self.sender.clone()
    }

    pub(crate) fn start_recv(mut self) {
        tokio::spawn(async move {
            info!("UserManager task spawned!");

            loop {
                while let Ok(msg) = self.receiver.try_recv() {
                    match msg {
                        UserManagerMessage::NOTIFY_USER(uuid, user_notification) => {
                            self.users.insert(uuid, user_notification);
                        },

                        UserManagerMessage::CLIENT_MESSAGE(notification) => match notification {
                            Notification::MESSAGE(chat_uuid, message) => todo!(),
                            Notification::NEW_CHAT(chat) => todo!(),
                            Notification::FRIEND_REQUEST(sender, receiver) => {
                                if let Some(receiver_notifications) = self.users.get(&receiver) {
                                    let _ = receiver_notifications.lock().await
                                        .sender.send(Notification::FRIEND_REQUEST(sender, UUID::default()));
                                }
                            }
                        }
                    };
                }
            
                // TODO: Update ChatManager
            }
        });
    }
}

pub(crate) struct ServerUser {
    pub(crate) user: User,
    pub(crate) sender: Sender<Notification>,
    pub(crate) receiver: Receiver<Notification>,
    pub(crate) chat_receivers: Vec<tokio::sync::broadcast::Receiver<ServerMessage>>,
}
impl ServerUser {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        
        Self {
            user: User::default(),
            sender,
            receiver,
            chat_receivers: Vec::default(),
        }
    }
}