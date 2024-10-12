use std::process::ExitStatus;
use mongodb::bson::doc;
use yapping_core::{l3gion_rust::{sllog::warn, StdError, UUID}, user::{DbUser, User, UserCreationInfo}};

const MONGO_PATH: &str = "mongoDB/core/bin/mongod.exe";
const MONGO_DATA: &str = "mongoDB/data";
const MONGO_LOG: &str = "mongoDB/log";

pub(crate) struct MongoDB {
    _db_thread: std::thread::JoinHandle<std::io::Result<ExitStatus>>,
    mongo_client: mongodb::Client,
    mongo_database: mongodb::Database,
}
// Public(crate)
impl MongoDB {
    pub(crate) async fn new() -> Result<Self, StdError> {
        // MongoDB setup
        std::fs::create_dir_all(MONGO_DATA).map_err(|_| "Failed to create mongodb data dir!")?;
        
        let _db_thread = std::thread::spawn(move || {
           let mut child = std::process::Command::new(MONGO_PATH)
            .arg("--dbpath")
            .arg(MONGO_DATA)
            .arg("--logpath")
            .arg(MONGO_LOG)
            .spawn()
            .expect("Failed to run mongod command!");
        
            child.wait()
        });

        // Client initialization.
        // TODO: Make this better.
        let client_op = mongodb::options::ClientOptions::parse("mongodb://localhost:27017/?directConnection=true").await?;
        let mongo_client = mongodb::Client::with_options(client_op)?;

        // Connecting to database.
        let mongo_database = mongo_client.database("yapping_db");

        Ok(Self {
            _db_thread,
            mongo_client,
            mongo_database,
        })
    }

    pub(crate) async fn login(&self, info: UserCreationInfo) -> Result<User, StdError> {
        let db_user = self.get_user(doc! { 
            "email": info.email.clone(),
            "password": info.password.to_string(),
        }).await?;

        Ok(User::from(db_user)?)
    }

    pub(crate) async fn sign_up(&self, info: UserCreationInfo) -> Result<User, StdError> {
        if self.get_user(doc! { "email": info.email.clone() }).await.is_ok() {
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

    pub(crate) async fn get_user(&self, document: mongodb::bson::Document) -> Result<DbUser, StdError> {
        let users = self.user_collection();
        users.find_one(document).await?.ok_or("Failed to find User!".into())
    }
}
// Private
impl MongoDB {
    fn user_collection(&self) -> mongodb::Collection<DbUser> {
        self.mongo_database.collection::<DbUser>("Users")
    }
}