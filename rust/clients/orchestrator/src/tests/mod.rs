use anyhow::Context;
use anyhow::Result;
use async_nats::{jetstream, Client, ConnectOptions};
use mongodb::{options::ClientOptions, Client as MongoClient};
use rand::Rng;
use std::{path::PathBuf, process::Command, str::FromStr, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::time::sleep;

pub mod workloads;

// Helper function to create a test MongoDB instance
pub struct MongodRunner {
    _child: std::process::Child,
    // this is stored to prevent premature removing of the tempdir
    tempdir: TempDir,
}

impl MongodRunner {
    fn socket_path(tempdir: &TempDir) -> anyhow::Result<String> {
        Ok(format!(
            "{}/mongod.sock",
            tempdir
                .path()
                .canonicalize()?
                .as_mut_os_str()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("can't convert path to str"))?
        ))
    }

    pub fn run() -> anyhow::Result<Self> {
        let tempdir = TempDir::new().unwrap();

        std::fs::File::create_new(Self::socket_path(&tempdir)?)?;

        let mut cmd = std::process::Command::new("mongod");
        cmd.args([
            "--unixSocketPrefix",
            &tempdir.path().to_string_lossy(),
            "--dbpath",
            &tempdir.path().to_string_lossy(),
            "--bind_ip",
            &Self::socket_path(&tempdir)?,
            "--port",
            &0.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

        let child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to spawn {cmd:?}: {e}"));

        let new_self = Self {
            _child: child,
            tempdir,
        };

        std::fs::exists(Self::socket_path(&new_self.tempdir)?)
            .context("mongod socket should exist")?;
        println!(
            "MongoDB Server is running at {:?}",
            new_self.socket_pathbuf()
        );

        Ok(new_self)
    }

    fn socket_pathbuf(&self) -> anyhow::Result<PathBuf> {
        Ok(PathBuf::from_str(&Self::socket_path(&self.tempdir)?)?)
    }

    pub fn client(&self) -> anyhow::Result<MongoClient> {
        let server_address = mongodb::options::ServerAddress::Unix {
            path: self.socket_pathbuf()?,
        };
        let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
        Ok(MongoClient::with_options(client_options)?)
    }
}

pub struct TestClientResponse {
    client: Client,
    _js: jetstream::Context,
}

pub struct TestNatsServer {
    _temp_dir: TempDir,
    _process: Arc<tokio::process::Child>,
    pub port: String,
}

impl TestNatsServer {
    /// Spin up NATS server
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let jetstream_dir = temp_dir.path().join("jetstream");
        std::fs::create_dir_all(&jetstream_dir)?;

        // Start NATS server with JetStream enabled
        let mut port = generate_random_port();
        println!("proposed port: {port}");

        let is_port_availabe = check_port_availability(&port).await;
        println!("is_port_availabe: {is_port_availabe:?}");

        while is_port_availabe.is_err() {
            println!("Port {port} is not available -- attempting to find a new port.");
            port = generate_random_port();
            sleep(Duration::from_secs(1)).await;
        }
        println!("spinning up server on port: {port}");

        let process = tokio::process::Command::new("nats-server")
            .args([
                "--jetstream",
                "--store_dir",
                jetstream_dir.to_str().unwrap(),
                "--port",
                &port,
            ])
            .kill_on_drop(true)
            .spawn()?;

        let server = Self {
            _temp_dir: temp_dir,
            _process: Arc::new(process),
            port,
        };

        // Wait for server to start
        sleep(Duration::from_secs(1)).await;

        Ok(server)
    }

    /// Connect client to NATS server
    pub async fn connect(&self, port: &str) -> Result<TestClientResponse> {
        let client = ConnectOptions::new()
            .name("test_orchestrator_client")
            .connect(&format!("nats://localhost:{}", port))
            .await?;

        Ok(TestClientResponse {
            client: client.clone(),
            _js: jetstream::new(client),
        })
    }

    /// Form the url of the nats server
    pub fn get_url(&self) -> String {
        format!("nats://localhost:{}", self.port)
    }

    /// Gracefully shut down the NATS server
    pub async fn shutdown(self) -> Result<()> {
        if let Ok(mut child) = Arc::try_unwrap(self._process) {
            child.kill().await?;
            let status = child.wait().await?;
            if !status.success() {
                return Err(anyhow::anyhow!("Failed to shut down NATS server"));
            }
        }

        // Wait for the port to be free
        wait_for_port_release(&self.port).await?;

        Ok(())
    }
}

// Helper function to check that a port is available
async fn check_port_availability(port: &str) -> Result<()> {
    let output = Command::new("lsof")
        .arg("-i")
        .arg(format!("{:?}", port))
        .output()?;

    if output.stdout.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Port is in use"))
    }
}

// Helper function to get a random port
fn generate_random_port() -> String {
    let mut rng = rand::rng();
    let port = rng.random_range(4444..5555);
    port.to_string()
}

// Helper function to wait for a port to be available
async fn wait_for_port_release(port: &str) -> Result<()> {
    let max_retries = 10;
    let mut retries = 0;

    while retries < max_retries {
        // Check that the port is free
        if check_port_availability(port).await.is_ok() {
            return Ok(());
        }
        retries += 1;
        sleep(Duration::from_secs(1)).await;
    }

    Err(anyhow::anyhow!(
        "Port {} is still occupied after {} retries",
        port,
        max_retries
    ))
}
// Helper function to check if nats-server is available
pub fn check_nats_server() -> bool {
    Command::new("nats-server")
        .arg("--version")
        .output()
        .is_ok()
}
