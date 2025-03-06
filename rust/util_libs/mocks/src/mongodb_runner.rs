/// This module implements running ephemeral Mongod instances.
/// It disables TCP and relies only unix domain sockets.
use anyhow::Context;
use mongodb::{options::ClientOptions, Client};
use std::{path::PathBuf, process::Stdio, str::FromStr};
use tempfile::TempDir;

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
        let tempdir = TempDir::new().context("Failed to create tempdir.")?;

        let socket_path = Self::socket_path(&tempdir)?;

        // Ensure socket file does not exist
        let socket_file = PathBuf::from(&socket_path);
        if socket_file.exists() {
            std::fs::remove_file(&socket_file)?;
        }

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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        let child = cmd.spawn().context("Failed to start mongod")?;

        let new_self = Self {
            _child: child,
            tempdir,
        };

        // Wait for db to be ready
        let retries = 10;
        for _ in 0..retries {
            if socket_file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        }

        if !socket_file.exists() {
            return Err(anyhow::anyhow!(
                "MongoDB did not create the socket file in time"
            ));
        }

        println!(
            "MongoDB Server is running at {:?}",
            new_self.get_socket_pathbuf()
        );

        Ok(new_self)
    }

    pub fn get_socket_pathbuf(&self) -> anyhow::Result<PathBuf> {
        Ok(PathBuf::from_str(&Self::socket_path(&self.tempdir)?)?)
    }

    pub fn client(&self) -> anyhow::Result<Client> {
        let server_address = mongodb::options::ServerAddress::Unix {
            path: self.get_socket_pathbuf()?,
        };
        let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
        Ok(Client::with_options(client_options)?)
    }
}
