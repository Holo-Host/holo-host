use anyhow::{Context, Result};
use axum::{
    extract::Json,
    http::StatusCode,
    response::Json as JsonResponse,
    routing::{get, post},
    Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind to
    #[arg(long, default_value = "5000")]
    port: u16,

    /// Authentication key for requests
    #[arg(long)]
    auth_key: String,

    /// NSC path
    #[arg(long, default_value = "/var/lib/nats_server/.local/share/nats/nsc")]
    nsc_path: String,
}

#[derive(Debug, Deserialize)]
struct NSCRequest {
    command: String,
    params: NSCParams,
    auth_key: String,
}

#[derive(Debug, Deserialize)]
struct NSCParams {
    account: Option<String>,
    name: Option<String>,
    key: Option<String>,
    role: Option<String>,
    tag: Option<String>,
    field: Option<String>,
    output_file: Option<String>,
}

#[derive(Debug, Serialize)]
struct NSCResponse {
    success: bool,
    stdout: String,
    stderr: String,
    returncode: i32,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
}

#[derive(Debug, Clone)]
struct AppState {
    auth_key: String,
    nsc_path: String,
}

#[derive(Debug)]
enum AllowedCommand {
    AddUser,
    DescribeUser,
    GenerateCreds,
}

impl AllowedCommand {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "add_user" => Some(AllowedCommand::AddUser),
            "describe_user" => Some(AllowedCommand::DescribeUser),
            "generate_creds" => Some(AllowedCommand::GenerateCreds),
            _ => None,
        }
    }

    fn get_args(&self) -> Vec<&'static str> {
        match self {
            AllowedCommand::AddUser => vec!["add", "user"],
            AllowedCommand::DescribeUser => vec!["describe", "user"],
            AllowedCommand::GenerateCreds => vec!["generate", "creds"],
        }
    }

    fn get_required_params(&self) -> Vec<&'static str> {
        match self {
            AllowedCommand::AddUser => vec!["account", "name", "key"],
            AllowedCommand::DescribeUser => vec!["account", "name"],
            AllowedCommand::GenerateCreds => vec!["account", "name"],
        }
    }

    fn get_optional_params(&self) -> Vec<&'static str> {
        match self {
            AllowedCommand::AddUser => vec!["role", "tag"],
            AllowedCommand::DescribeUser => vec!["field"],
            AllowedCommand::GenerateCreds => vec!["output_file"],
        }
    }
}

fn validate_request(request: &NSCRequest) -> Result<AllowedCommand> {
    let command = AllowedCommand::from_str(&request.command)
        .ok_or_else(|| anyhow::anyhow!("Command '{}' not allowed", request.command))?;

    // Validate required parameters
    let required_params = command.get_required_params();
    for param in required_params {
        match param {
            "account" if request.params.account.is_none() => {
                return Err(anyhow::anyhow!("Missing required parameter: account"));
            }
            "name" if request.params.name.is_none() => {
                return Err(anyhow::anyhow!("Missing required parameter: name"));
            }
            "key" if request.params.key.is_none() => {
                return Err(anyhow::anyhow!("Missing required parameter: key"));
            }
            _ => {}
        }
    }

    Ok(command)
}

fn build_nsc_command(command: &AllowedCommand, params: &NSCParams) -> Vec<String> {
    let mut args = command.get_args().iter().map(|s| s.to_string()).collect::<Vec<_>>();

    // Add required parameters
    if let Some(account) = &params.account {
        args.extend(vec!["-a".to_string(), account.clone()]);
    }
    if let Some(name) = &params.name {
        args.extend(vec!["-n".to_string(), name.clone()]);
    }
    if let Some(key) = &params.key {
        args.extend(vec!["-k".to_string(), key.clone()]);
    }

    // Add optional parameters
    if let Some(role) = &params.role {
        args.extend(vec!["-K".to_string(), role.clone()]);
    }
    if let Some(tag) = &params.tag {
        args.extend(vec!["--tag".to_string(), tag.clone()]);
    }
    if let Some(field) = &params.field {
        args.extend(vec!["--field".to_string(), field.clone()]);
    }
    if let Some(output_file) = &params.output_file {
        args.extend(vec!["--output-file".to_string(), output_file.clone()]);
    }

    args
}

async fn execute_nsc_command(args: Vec<String>, nsc_path: &str) -> Result<NSCResponse> {
    let mut command = Command::new("nsc");
    command.args(&args);
    command.env("NSC_PATH", nsc_path);

    // Execute with timeout
    let output = timeout(
        Duration::from_secs(30),
        tokio::task::spawn_blocking(move || command.output()),
    )
    .await
    .context("NSC command timed out")?
    .context("Failed to execute NSC command")?;

    let output = output.unwrap();
    Ok(NSCResponse {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        returncode: output.status.code().unwrap_or(-1),
    })
}

async fn nsc_proxy_handler(
    state: axum::extract::State<AppState>,
    Json(request): Json<NSCRequest>,
) -> Result<JsonResponse<NSCResponse>, (StatusCode, JsonResponse<ErrorResponse>)> {
    // Authenticate request
    if request.auth_key != state.auth_key {
        warn!("Authentication failed for request");
        return Err((
            StatusCode::UNAUTHORIZED,
            JsonResponse(ErrorResponse {
                error: "Authentication failed".to_string(),
            }),
        ));
    }

    // Validate request
    let command = validate_request(&request).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            JsonResponse(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Build NSC command
    let nsc_args = build_nsc_command(&command, &request.params);

    // Execute command
    let result = execute_nsc_command(nsc_args, &state.nsc_path).await.map_err(|e| {
        error!("Failed to execute NSC command: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            JsonResponse(ErrorResponse {
                error: format!("Failed to execute NSC command: {}", e),
            }),
        )
    })?;

    Ok(JsonResponse(result))
}

async fn health_handler() -> JsonResponse<HealthResponse> {
    JsonResponse(HealthResponse {
        status: "healthy".to_string(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = Args::parse();

    // Validate NSC path
    if !std::path::Path::new(&args.nsc_path).exists() {
        return Err(anyhow::anyhow!("NSC path does not exist: {}", args.nsc_path));
    }

    // Check if NSC is available
    let nsc_check = Command::new("nsc")
        .arg("--version")
        .output()
        .context("Failed to check NSC availability")?;

    if !nsc_check.status.success() {
        return Err(anyhow::anyhow!("NSC command not found or not working"));
    }

    // Create app state
    let state = AppState {
        auth_key: args.auth_key,
        nsc_path: args.nsc_path,
    };

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/nsc", post(nsc_proxy_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", args.host, args.port);
    info!("Starting NSC proxy server on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
} 