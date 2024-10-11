use std::process::ExitStatus;
use serde::{Deserialize, Serialize};
use yapping_core::l3gion_rust::{sllog::warn, StdError, UUID};

const MONGO_PATH: &str = "mongoDB/core/bin/mongod.exe";
const MONGO_DATA: &str = "mongoDB/data";
const MONGO_LOG: &str = "mongoDB/log";

#[derive(Serialize, Deserialize)]
pub(crate) struct User {
    _id: String,
    user_tag: String,
    email: String,
    password: String,
    user_pic: String,
}

pub(crate) struct MongoDB {
    _db_thread: std::thread::JoinHandle<std::io::Result<ExitStatus>>,
    mongo_client: mongodb::Client,
    mongo_database: mongodb::Database,
}
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
    
    pub(crate) async fn new_user(&self) {
        let user = User {
            _id: UUID::generate().to_string(),
            user_tag: "L3gion".to_string(),
            email: "legion@gmail.com".to_string(),
            password: UUID::generate().to_string(),
            user_pic: UUID::generate().to_string(),
        };
        
        let users = self.mongo_database.collection::<User>("Users");
        
        let result = users.insert_one(user).await.unwrap();
        warn!("{:?}", result);
    }
}