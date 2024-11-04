use std::process::ExitStatus;
use mongodb::bson::doc;
use yapping_core::{l3gion_rust::{StdError, UUID}, user::{DbUser, User, UserCreationInfo}};
use futures::StreamExt;
use mongodb::{Client, Database};
use std::io::Error as IoError;
use std::process::Command;
use tokio::task;

const MONGO_DATA: &str = "mongo_db/data";
const MONGO_LOG: &str = "mongo_db/log";

pub(crate) struct MongoDBClient {
    _db_thread: tokio::task::JoinHandle<Result<ExitStatus, IoError>>,
    mongo_client: Client,
}
impl MongoDBClient {
    pub(crate) async fn new() -> Result<Self, StdError> {
        std::fs::create_dir_all(MONGO_DATA).map_err(|_| "Failed to create MongoDB data directory!")?;

        let _db_thread = task::spawn_blocking(move || {
            Command::new("mongod")
                .arg("--dbpath")
                .arg(MONGO_DATA)
                .arg("--logpath")
                .arg(MONGO_LOG)
                .spawn()
                .and_then(|mut child| child.wait())
        });

        let client_options = mongodb::options::ClientOptions::parse("mongodb://localhost:27017/?directConnection=true")
            .await
            .map_err(|e| format!("Failed to parse client options: {}", e))?;
        
        let mongo_client = Client::with_options(client_options)
            .map_err(|e| format!("Failed to create MongoDB client: {}", e))?;
        
        Ok(Self {
            _db_thread,
            mongo_client,
        })
    }
    
    pub(crate) fn get_database(&self) -> MongoDB {
        MongoDB(self.mongo_client.database("yapping_db"))
    }
}

pub(crate) struct MongoDB(Database);
impl MongoDB {
    pub(crate) async fn login(&self, info: UserCreationInfo) -> Result<User, StdError> {
        let db_user = self.get_db_user(doc! { 
            "email": info.email.clone(),
            "password": info.password.to_string(),
        }).await?;

        let mut friends = Vec::with_capacity(db_user.friends().len());
        for friend_uuid in db_user.friends() {
            friends.push(User::from(self.get_db_user(doc! {
                "_id": friend_uuid
            }).await?)?);
        }

        let mut user = User::from(db_user)?;
        user.set_friends(friends);

        Ok(user)
    }

    pub(crate) async fn sign_up(&self, info: UserCreationInfo) -> Result<User, StdError> {
        if self.get_db_user(doc! { "email": info.email.clone() }).await.is_ok() {
            return Err("User already exist!".into());
        }
        else if info.tag.is_empty() || info.email.is_empty() || !info.password.is_valid() {
            return Err("Please fill all the fields!".into());
        }
        
        let db_user = DbUser::new(info);
        let users = self.user_collection();
        
        users.insert_one(db_user.clone()).await?;

        Ok(User::from(db_user)?)
    }

    pub(crate) async fn get_db_user(&self, document: mongodb::bson::Document) -> Result<DbUser, StdError> {
        let users = self.user_collection();
        users.find_one(document).await?.ok_or("Failed to find User!".into())
    }

    pub(crate) async fn get_striped_user(&self, document: mongodb::bson::Document) -> Result<User, StdError> {
        self.get_db_user(document).await
            .and_then(|db_user| User::from(db_user))
            .map(|mut user| {
                user.strip_info();
                
                user
            })
    }
    
    pub(crate) async fn query_by_tag(&self, tags: Vec<String>) -> Vec<User> {
        let mut result = Vec::with_capacity(tags.len());
        
        for tag in tags {
            if let Ok(user) = self.get_striped_user(doc! { "tag": tag }).await {
                result.push(user);
            }
        }
        
        result
    }
    
    pub(crate) async fn query_contains_tag(&self, tag: String) -> Result<Vec<User>, StdError> {
        let users = self.user_collection();

        let users = users.find(doc! {
            "tag": { "$regex": tag, "$options": "i" } // "i" option for case-insensitive search
        }).await?
        .collect::<Vec<Result<DbUser, _>>>().await
        .into_iter()
        .filter_map(|db_user| db_user.ok())
        .filter_map(|db_user| User::from(db_user).ok())
        .map(|mut user| { 
            user.strip_info();
            user
        })
        .collect();
        
        Ok(users)
    }
    
    pub(crate) async fn query_by_uuid(&self, uuids: Vec<UUID>) -> Vec<User> {
        let mut result = Vec::with_capacity(uuids.len());
        
        for uuid in uuids {
            if let Ok(user) = self.get_striped_user(doc! { "_id": uuid.to_string() }).await {
                result.push(user);
            }
        }
        
        result
    }
}
// Private
impl MongoDB {
    fn user_collection(&self) -> mongodb::Collection<DbUser> {
        self.0.collection::<DbUser>("Users")
    }
}