use anyhow::Result;
use async_nats::{
    jetstream::{self, Context},
    Client, ConnectOptions,
};
use rand::Rng;
use std::{process::Command, sync::Arc};
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

pub struct TestClientResponse {
    pub client: Client,
    pub js: Context,
}

pub struct TestNatsServer {
    _temp_dir: TempDir,
    _process: Arc<tokio::process::Child>,
    pub port: String,
}

impl TestNatsServer {
    /// Spin up NATS server
    pub async fn new() -> Result<Self> {
        // Check NATS availability first
        if !check_nats_server() {
            return Err(anyhow::anyhow!("NATS server not available"));
        }

        let temp_dir = TempDir::new()?;
        let jetstream_dir = temp_dir.path().join("jetstream");
        std::fs::create_dir_all(&jetstream_dir)?;

        let mut port = String::new();
        let mut process = None;
        let max_attempts = 5;
        let mut attempts = 0;

        while attempts < max_attempts {
            port = generate_random_port();
            log::info!("Attempting to start NATS server on port: {port}");

            // Start NATS server with JetStream enabled
            let spawn_result = tokio::process::Command::new("nats-server")
                .args([
                    "--jetstream",
                    "--store_dir",
                    jetstream_dir.to_str().unwrap(),
                    "--port",
                    &port,
                ])
                .kill_on_drop(true)
                .spawn();

            match spawn_result {
                Ok(p) => {
                    process = Some(p);
                    // Wait a bit for the server to be ready
                    sleep(Duration::from_secs(1)).await;

                    // Try to connect to verify the server is up
                    match tokio::time::timeout(Duration::from_secs(2), async {
                        let connect_result = ConnectOptions::new()
                            .name("test_client")
                            .connect(&format!("nats://localhost:{}", port))
                            .await;
                        if connect_result.is_ok() {
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!("Failed to connect to test server"))
                        }
                    })
                    .await
                    {
                        Ok(Ok(_)) => break,
                        _ => {
                            if let Some(mut p) = process.take() {
                                let _ = p.kill().await;
                            }
                            attempts += 1;
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                }
                Err(_) => {
                    attempts += 1;
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        }

        let process = process.ok_or_else(|| anyhow::anyhow!("Failed to start NATS server"))?;

        Ok(Self {
            _temp_dir: temp_dir,
            _process: Arc::new(process),
            port,
        })
    }

    /// Connect client to NATS server
    pub async fn connect(&self, port: &str) -> Result<TestClientResponse> {
        let timeout = Duration::from_secs(5);
        match tokio::time::timeout(
            timeout,
            ConnectOptions::new()
                .name("test_client")
                .connect(&format!("nats://localhost:{}", port)),
        )
        .await
        {
            Ok(Ok(client)) => Ok(TestClientResponse {
                client: client.clone(),
                js: jetstream::new(client),
            }),
            Ok(Err(e)) => Err(anyhow::anyhow!("Failed to connect to NATS: {}", e)),
            Err(_) => Err(anyhow::anyhow!("Connection timed out after {:?}", timeout)),
        }
    }

    /// Gracefully shut down the NATS server
    pub async fn shutdown(self) -> Result<()> {
        if let Ok(mut child) = Arc::try_unwrap(self._process) {
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        }
        log::info!("NATS server successfully shut down...");
        Ok(())
    }
}

// Helper function to get a random port
fn generate_random_port() -> String {
    let mut rng = rand::rng();
    rng.random_range(4444..5555).to_string()
}

// Helper function to check that the nats-server is available
pub fn check_nats_server() -> bool {
    Command::new("nats-server")
        .arg("--version")
        .output()
        .is_ok()
}
