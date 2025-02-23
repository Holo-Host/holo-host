use anyhow::Result;
use async_nats::{
    jetstream::{self, Context},
    Client, ConnectOptions,
};
use std::fs::File;
use std::io::Write;
use std::{fs, path::Path, process::Command, sync::Arc};
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

pub mod jetstream_client;
pub mod jetstream_service;
pub mod leaf_server;

const JWT_TEST_DIR: &str = "./jwt";
const TEMP_DIR: &str = "./tmp";
const LOCAL_DIR: &str = "./.local";
const TEST_AUTH_DIR: &str = "./tmp/test-auth";
const RESOLVER_FILE_PATH: &str = "./test_configs/resolver.conf";
const OPERATOR_NAME: &str = "test-operator";
const USER_ACCOUNT_NAME: &str = "host-account";
const USER_NAME: &str = "host-user";
const NSC_CREDS_PATH: &str = ".local/share/nats/nsc/keys/creds";

pub struct TestServerResponse {
    client: Client,
    js: Context,
}

pub struct TestNatsServer {
    _temp_dir: TempDir,
    _process: Arc<tokio::process::Child>,
    pub port: String,
}

impl TestNatsServer {
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let jetstream_dir = temp_dir.path().join("jetstream");
        std::fs::create_dir_all(&jetstream_dir)?;

        // Start NATS server with JetStream enabled
        let port = "4444".to_string();
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

    /// Spin up NATS server
    pub async fn connect(&self, port: &str) -> Result<TestServerResponse> {
        let client = ConnectOptions::new()
            .name("test_client")
            .connect(&format!("nats://localhost:{}", port))
            .await?;

        Ok(TestServerResponse {
            client: client.clone(),
            js: jetstream::new(client),
        })
    }

    /// Gracefully shut down the NATS server
    pub async fn shutdown(self) -> Result<()> {
        if let Ok(mut child) = Arc::try_unwrap(self._process) {
            child.kill().await?;
            child.wait().await?;
        }
        Ok(())
    }
}

// Helper function to check if nats-server is available
pub fn check_nats_server() -> bool {
    Command::new("nats-server")
        .arg("--version")
        .output()
        .is_ok()
}

// TODO: Clean up this function to abstract away redundacy and improve readability
// Generates Operator, SYS account with user, and holo-account Account with user,
// then instantiates the nats resolver and generates the jwt and creds needed
// for the Leaf server to connect to Hub with JWT Chain-Of-Trust auth
pub fn gen_test_agents(jwt_server_url: &str) {
    if Path::new(JWT_TEST_DIR).exists() {
        fs::remove_dir_all(JWT_TEST_DIR).expect("Failed to delete already existing jwt dir");
    }
    fs::create_dir_all(JWT_TEST_DIR).expect("Failed to create jwt dir");

    if Path::new(TEST_AUTH_DIR).exists() {
        fs::remove_dir_all(TEST_AUTH_DIR).expect("Failed to delete already existing test auth dir");
    }
    fs::create_dir_all(TEST_AUTH_DIR).expect("Failed to create test auth dir");

    if Path::new(RESOLVER_FILE_PATH).exists() {
        fs::remove_file(RESOLVER_FILE_PATH)
            .expect("Failed to delete already existing resolver file");
    }

    if Path::new(NSC_CREDS_PATH).exists() {
        fs::remove_dir_all(NSC_CREDS_PATH)
            .expect("Failed to delete already existing nsc creds dir");
    }
    fs::create_dir_all(NSC_CREDS_PATH).expect("Failed to create nsc creds dir");

    let nsc_sys_account_path = format!("{}/{}/SYS", NSC_CREDS_PATH, OPERATOR_NAME);
    fs::create_dir_all(&nsc_sys_account_path).expect("Failed to create nsc creds dir");

    let nsc_user_account_path = format!(
        "{}/{}/{}/",
        NSC_CREDS_PATH, OPERATOR_NAME, USER_ACCOUNT_NAME
    );
    fs::create_dir_all(&nsc_user_account_path).expect("Failed to create nsc creds dir");

    // Create operator and sys account/user
    Command::new("nsc")
        .args(["env", "-s", TEST_AUTH_DIR])
        .output()
        .expect("Failed to set env to the test auth dir");

    Command::new("nsc")
        .args(["add", "operator", "-n", OPERATOR_NAME, "--sys"])
        .output()
        .expect("Failed to add operator");

    Command::new("nsc")
        .args([
            "edit",
            "operator",
            "--account-jwt-server-url",
            &format!("nats://{}", jwt_server_url),
        ])
        .output()
        .expect("Failed to create edit operator");

    // Create host account (with js enabled)
    Command::new("nsc")
        .args(["add", "account", USER_ACCOUNT_NAME])
        .output()
        .expect("Failed to add acccount");

    Command::new("nsc")
        .args(["edit", "account", USER_ACCOUNT_NAME])
        .args([
            "--sk generate",
            "--js-streams -1",
            "--js-consumer -1",
            "--js-mem-storage 1G",
            "--js-disk-storage 512",
        ])
        .output()
        .expect("Failed to create edit account");

    // Create user for host account
    Command::new("nsc")
        .args(["add", "user", USER_NAME])
        .args(["--account", USER_ACCOUNT_NAME])
        .output()
        .expect("Failed to add user");

    // Generate resolver file and create resolver file
    Command::new("nsc")
        .args([
            "generate",
            "config",
            "--nats-resolver",
            "--sys-account",
            "SYS",
            "--force",
            "--config-file",
            RESOLVER_FILE_PATH,
        ])
        .output()
        .expect("Failed to create resolver config file");

    let nsc_sys_creds_path = format!("{}/{}/SYS/sys.creds", NSC_CREDS_PATH, OPERATOR_NAME);
    Command::new("nsc")
        .args([
            "generate",
            "creds",
            "--name",
            "sys",
            "--account",
            "SYS",
            "--output-file",
            &nsc_sys_creds_path,
        ])
        .output()
        .expect("Failed to add sys user key to hosting agent");
    log::debug!("nsc_sys_creds_path : {}", nsc_sys_creds_path);

    let nsc_user_creds_path = format!(
        "{}/{}/{}/{}.creds",
        NSC_CREDS_PATH, OPERATOR_NAME, USER_ACCOUNT_NAME, USER_NAME
    );
    Command::new("nsc")
        .args([
            "generate",
            "creds",
            "--name",
            USER_NAME,
            "--account",
            USER_ACCOUNT_NAME,
            "--output-file",
            &nsc_user_creds_path,
        ])
        .output()
        .expect("Failed to add sys user key to hosting agent");
    log::debug!("nsc_user_creds_path : {}", nsc_user_creds_path);

    let sys_account_output = Command::new("nsc")
        .args(["describe", "account", "SYS", "--field", "sub"])
        .output()
        .expect("Failed to execute nsc command");
    let sys_account_pubkey = std::str::from_utf8(&sys_account_output.stdout)
        .expect("Invalid UTF-8 output")
        .trim()
        .trim_matches('"');

    let mut output = Command::new("nsc")
        .args(["describe", "account", "--name", "SYS", "--raw"])
        .output()
        .expect("Failed to execute nsc command");
    if !output.status.success() {
        log::debug!("Command failed with status: {}", output.status);
        std::process::exit(1);
    } else {
        // Read the command output and filter out lines containing "-----"
        let output_str = String::from_utf8_lossy(&output.stdout);
        let filtered_lines: Vec<String> = output_str
            .lines()
            .filter(|line| !line.contains("-----")) // Remove JWT header/footer
            .map(String::from)
            .collect();
        let sys_account_jwt_path: String = format!("{}/{}.jwt", JWT_TEST_DIR, sys_account_pubkey);
        let mut file = File::create(&sys_account_jwt_path).expect("Failed to write SYS jwt file");
        for line in filtered_lines {
            writeln!(file, "{}", line).expect("Failed to write SYS jwt file");
        }
        log::debug!("SYS account JWT successfully written.");
    }

    let host_account_output = Command::new("nsc")
        .args(["describe", "account", USER_ACCOUNT_NAME, "--field", "sub"])
        .output()
        .expect("Failed to execute nsc command");
    let host_account_pubkey = std::str::from_utf8(&host_account_output.stdout)
        .expect("Invalid UTF-8 output")
        .trim()
        .trim_matches('"');

    output = Command::new("nsc")
        .args(["describe", "account", "--name", USER_ACCOUNT_NAME, "--raw"])
        .output()
        .expect("Failed to execute nsc command");
    if !output.status.success() {
        log::debug!("Command failed with status: {}", output.status);
        std::process::exit(1);
    } else {
        // Read the command output and filter out lines containing "-----"
        let output_str = String::from_utf8_lossy(&output.stdout);
        let filtered_lines: Vec<String> = output_str
            .lines()
            .filter(|line| !line.contains("-----")) // Remove JWT header/footer
            .map(String::from)
            .collect();
        let user_account_jwt_path: String = format!("{}/{}.jwt", JWT_TEST_DIR, host_account_pubkey);
        let mut file =
            File::create(&user_account_jwt_path).expect("Faileed to write host-account jwt file");
        for line in filtered_lines {
            writeln!(file, "{}", line).expect("Faileed to write host-account jwt file");
        }
        log::debug!("User account JWT successfully written.");
    }
}
