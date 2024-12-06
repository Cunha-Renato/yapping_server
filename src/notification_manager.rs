use std::collections::HashMap;

use crate::chat_manager::ChatManager;

use tokio::sync::mpsc::{Receiver, Sender};
use yapping_core::{client_server_coms::{Notification, NotificationType}, l3gion_rust::{sllog::{error, info}, UUID}};

#[allow(non_camel_case_types)]
pub(crate) enum NotificationManagerMessage {
    NOTIFY_USER(UUID, Vec<UUID>, Sender<Notification>),
    REFRESH_USER(UUID),
    USER_OFFLINE,
    CLIENT_MESSAGE(Notification),
}

pub(crate) struct NotificationManager {
    sender: Sender<(UUID, NotificationManagerMessage)>,
    receiver: Receiver<(UUID, NotificationManagerMessage)>,
    
    users: HashMap<UUID, Sender<Notification>>,
    chat_users: HashMap<UUID, Vec<tokio::sync::broadcast::Receiver<Notification>>>,

    chat_manager: ChatManager,
}
impl NotificationManager {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(100);

        Self {
            sender,
            receiver,
            users: HashMap::default(),
            chat_users: HashMap::default(),
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
                        NotificationManagerMessage::NOTIFY_USER(user_uuid, user_chats, user_notification) => {
                            self.users.insert(user_uuid, user_notification);
                            
                            for chat in user_chats {
                                self.chat_manager.new_chat(chat);
                                if let Some(receiver) = self.chat_manager.subscribe(chat) {
                                    self.chat_users.entry(user_uuid).or_insert(vec![])
                                        .push(receiver);
                                }
                            }
                        },
                        NotificationManagerMessage::REFRESH_USER(user_uuid) => {
                            if let Some(user_sender) = self.users.get(&user_uuid) {
                                if let Err(e) = user_sender.send(Notification::new(NotificationType::RESEND_USER(UUID::default()))).await {
                                    error!("In NotificationManager::start_recv::REFRESH_USER: {e}");
                                }
                            }
                        }
                        NotificationManagerMessage::USER_OFFLINE => {
                            self.users.remove(&sender_uuid);
                            self.chat_users.remove(&sender_uuid);
                        }

                        NotificationManagerMessage::CLIENT_MESSAGE(notification) => {
                            match notification.notification_type().clone() {
                                NotificationType::NEW_CHAT(chat) => {
                                    self.chat_manager.new_chat(chat.uuid());
                                    
                                    for u in chat.users() {
                                        if let Some(user_sender) = self.users.get(u) {
                                            if let Some(receiver) = self.chat_manager.subscribe(chat.uuid()) {
                                                self.chat_users.entry(*u).or_insert(vec![])
                                                    .push(receiver);
                                                
                                                if let Err(e) = user_sender.send(Notification::new(NotificationType::NEW_CHAT(chat.clone()))).await {
                                                    error!("In NotificationManager::start_recv::NEW_CHAT: {e}")
                                                }
                                            }
                                        }
                                    }
                                }
                                NotificationType::NEW_MESSAGE(chat_uuid, _) => self.chat_manager.post(chat_uuid, notification),
                                NotificationType::FRIEND_REQUEST(_, receiver) 
                                | NotificationType::FRIEND_ACCEPTED(_, receiver)=> {
                                    if let Some(receiving_user_sender) = self.users.get(&receiver) {
                                        if let Err(e) = receiving_user_sender.send(notification).await {
                                            error!("In NotificationManagerMessage::start_recv::FRIEND_ACCEPTED: {e}");
                                        }
                                    }
                                },
                                _ => (),
                        }
                    }};
                }
            
                // Bad? Yep bad. Slow? Very.
                for (chat_user, receivers) in &mut self.chat_users {
                    if let Some(sender) = self.users.get_mut(chat_user) {
                        for receiver in receivers {
                            while let Ok(notification) = receiver.try_recv() {
                                match notification.notification_type {
                                    NotificationType::NEW_MESSAGE(chat_uuid, msg) => if let Err(e) = sender
                                       .send(Notification::new(NotificationType::NEW_MESSAGE(chat_uuid, msg))).await 
                                    {
                                        error!("In NotificationManager::start_recv: {e}");
                                    }
                                    _ => error!("In NotificationManager::ChatManager update: Wrong NotificationType!"),
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}