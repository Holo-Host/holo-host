mod auth;
// mod workloads;
use anyhow::Result;
use dotenv::dotenv;
use tokio::task::spawn;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();
    println!("starting auth...");

    auth::run().await?;

    println!("finished auth...");

    // spawn(async move {
    //     if let Err(e) = workloads::run().await {
    //         log::error!("{}", e)
    //     }
    // });
    Ok(())
}
