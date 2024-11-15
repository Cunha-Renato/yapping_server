use std::collections::HashMap;

use crate::chat_manager::ChatManager;

use tokio::sync::mpsc::{Receiver, Sender};
use yapping_core::{client_server_coms::{Notification, NotificationType}, l3gion_rust::{sllog::{error, info}, UUID}};

#[allow(non_camel_case_types)]
pub(crate) enum NotificationManagerMessage {
    NOTIFY_USER(UUID, Sender<Notification>),
    USER_OFFLINE,
    CLIENT_MESSAGE(Notification),
}

pub(crate) struct NotificationManager {
    sender: Sender<(UUID, NotificationManagerMessage)>,
    receiver: Receiver<(UUID, NotificationManagerMessage)>,
    
    users: HashMap<UUID, Sender<Notification>>,
    chat_manager: ChatManager,
}
impl NotificationManager {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(100);

        Self {
            sender,
            receiver,
            users: HashMap::default(),
            chat_manager: ChatManager::default(),
        }
    }
    
    pub(crate) fn sender(&self) -> tokio::sync::mpsc::Sender<(UUID, NotificationManagerMessage)> {
        self.sender.clone()
    }

    pub(crate) fn start_recv(mut self) {
        tokio::spawn(async move {
            info!("UserManager task spawned!");

            loop {
                while let Ok((sender_uuid, msg)) = self.receiver.try_recv() {
                    match msg {
                        NotificationManagerMessage::NOTIFY_USER(user_uuid, user_notification) => {
                            self.users.insert(user_uuid, user_notification);
                        },
                        
                        NotificationManagerMessage::USER_OFFLINE => {
                            self.users.remove(&sender_uuid);
                        }

                        NotificationManagerMessage::CLIENT_MESSAGE(notification) => {
                            match notification.notification_type().clone() {
                                NotificationType::MESSAGE(chat_uuid, message) => todo!(),
                                NotificationType::MESSAGE_READ(_) => todo!(),
                                NotificationType::NEW_CHAT(chat) => todo!(),

                                NotificationType::FRIEND_REQUEST(_, receiver) 
                                | NotificationType::FRIEND_ACCEPTED(_, receiver)=> {
                                    if let Some(receiving_user_sender) = self.users.get(&receiver) {
                                        if let Err(e) = receiving_user_sender.send(notification).await {
                                            error!("{e}");
                                        }
                                    }
                                },
                        }
                    }};
                }
            
                // TODO: Update ChatManager
            }
        });
    }
}