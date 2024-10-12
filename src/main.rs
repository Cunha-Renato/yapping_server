use mongo_db::MongoDB;
use server_manager::ServerManager;

mod mongo_db;
mod server_manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        std::env::set_var("LOG", "4");
    }

    let manager = ServerManager::new(MongoDB::new().await?);
    manager.run().await?;
    
    Ok(())
}