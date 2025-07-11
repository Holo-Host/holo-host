use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct NSCRequest {
    pub command: String,
    pub params: NSCParams,
    pub auth_key: String,
}

#[derive(Debug, Serialize)]
pub struct NSCParams {
    pub account: Option<String>,
    pub name: Option<String>,
    pub key: Option<String>,
    pub role: Option<String>,
    pub tag: Option<String>,
    pub field: Option<String>,
    pub output_file: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NSCResponse {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub returncode: i32,
}

#[derive(Debug, Deserialize)]
pub struct NSCError {
    pub error: String,
}

pub struct NSCClient {
    client: Client,
    base_url: String,
    auth_key: String,
}

impl NSCClient {
    pub fn new(base_url: String, auth_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            base_url,
            auth_key,
        })
    }

    pub async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .context("Failed to send health check request")?;

        Ok(response.status().is_success())
    }

    pub async fn describe_user(
        &self,
        account: &str,
        name: &str,
        field: Option<&str>,
    ) -> Result<NSCResponse> {
        let params = NSCParams {
            account: Some(account.to_string()),
            name: Some(name.to_string()),
            key: None,
            role: None,
            tag: None,
            field: field.map(|f| f.to_string()),
            output_file: None,
        };

        self.execute_command("describe_user", params).await
    }

    pub async fn add_user(
        &self,
        account: &str,
        name: &str,
        key: &str,
        role: Option<&str>,
        tag: Option<&str>,
    ) -> Result<NSCResponse> {
        let params = NSCParams {
            account: Some(account.to_string()),
            name: Some(name.to_string()),
            key: Some(key.to_string()),
            role: role.map(|r| r.to_string()),
            tag: tag.map(|t| t.to_string()),
            field: None,
            output_file: None,
        };

        self.execute_command("add_user", params).await
    }

    pub async fn generate_creds(
        &self,
        account: &str,
        name: &str,
        output_file: Option<&str>,
    ) -> Result<NSCResponse> {
        let params = NSCParams {
            account: Some(account.to_string()),
            name: Some(name.to_string()),
            key: None,
            role: None,
            tag: None,
            field: None,
            output_file: output_file.map(|f| f.to_string()),
        };

        self.execute_command("generate_creds", params).await
    }

    async fn execute_command(&self, command: &str, params: NSCParams) -> Result<NSCResponse> {
        let request = NSCRequest {
            command: command.to_string(),
            params,
            auth_key: self.auth_key.clone(),
        };

        let response = self
            .client
            .post(format!("{}/nsc", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send NSC request")?;

        if response.status().is_success() {
            let nsc_response: NSCResponse = response
                .json()
                .await
                .context("Failed to parse NSC response")?;
            Ok(nsc_response)
        } else {
            let error_response: NSCError = response
                .json()
                .await
                .context("Failed to parse error response")?;
            Err(anyhow::anyhow!("NSC proxy error: {}", error_response.error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nsc_client_creation() {
        let client = NSCClient::new(
            "http://localhost:5000".to_string(),
            "test-auth-key".to_string(),
        );
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_nsc_request_serialization() {
        let request = NSCRequest {
            command: "add_user".to_string(),
            params: NSCParams {
                account: Some("HPOS".to_string()),
                name: Some("test_user".to_string()),
                key: Some("test_key".to_string()),
                role: Some("test_role".to_string()),
                tag: None,
                field: None,
                output_file: None,
            },
            auth_key: "test-auth-key".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("add_user"));
        assert!(json.contains("HPOS"));
        assert!(json.contains("test_user"));
    }
}
