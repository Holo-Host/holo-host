#![allow(dead_code)]

use anyhow::Context;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::{path::PathBuf, process::Stdio, str::FromStr};
use tempfile::TempDir;

/// This module implements running ephemeral Mongod instances.
/// It disables TCP and relies only unix domain sockets.
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
        .stdout(Stdio::null())
        .stderr(Stdio::null());

        let child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to spawn {cmd:?}: {e}"));

        let new_self = Self {
            _child: child,
            tempdir,
        };

        std::fs::exists(Self::socket_path(&new_self.tempdir)?)
            .context("mongod socket should exist")?;

        log::info!(
            "MongoDB Server is running at {:?}",
            new_self.socket_pathbuf()
        );

        Ok(new_self)
    }

    fn socket_pathbuf(&self) -> anyhow::Result<PathBuf> {
        Ok(PathBuf::from_str(&Self::socket_path(&self.tempdir)?)?)
    }

    pub fn client(&self) -> anyhow::Result<MongoDBClient> {
        let server_address = mongodb::options::ServerAddress::Unix {
            path: self.socket_pathbuf()?,
        };
        let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
        Ok(MongoDBClient::with_options(client_options)?)
    }
}
