use anyhow::Result;
use clap::Parser;
use ham::Ham;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The port number of the Holochain conductor's admin interface
    #[arg(short, long, default_value = "4444")]
    port: u16,

    /// Path or URL to the .happ file to install
    #[arg(short, long)]
    happ: String,

    /// Optional network seed for the app
    #[arg(short, long)]
    network_seed: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Convert network_seed string to NetworkSeed if provided
    let network_seed = cli.network_seed.map(|s| s.into());

    // Connect to the conductor
    let mut ham = Ham::connect(cli.port).await?;

    // Install and enable the app
    let app_info = ham
        .install_and_enable_with_default_agent(&cli.happ, network_seed)
        .await?;

    println!("Successfully installed app: {}", app_info.installed_app_id);

    Ok(())
}
