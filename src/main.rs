mod web_socket;
mod mongo_db;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        std::env::set_var("LOG", "4");
    }
    let mongo_db = mongo_db::MongoDB::new().await?;
    mongo_db.new_user().await;

    web_socket::run().await?;
    
    Ok(())
}