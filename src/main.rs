use server_manager::ServerManager;
use yapping_core::l3gion_rust::StdError;

mod mongo_db;
mod server_manager;
mod notification_manager;
mod chat_manager;
mod coms;

#[tokio::main]
async fn main() -> Result<(), StdError> {
    if cfg!(debug_assertions) {
        std::env::set_var("LOG", "4");
    }

    let manager = ServerManager::new().await?;
    manager.run().await?;
    
    Ok(())
}