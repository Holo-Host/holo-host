#![allow(dead_code)]

use anyhow::Context;
use mongodb::{options::ClientOptions, Client as MongoDBClient, Database};
use std::env;
use uuid::Uuid;

// This struct connects to MongoDB Atlas for testing
pub struct MongodRunner {
    client: MongoDBClient,
    db_name: String,
}

impl MongodRunner {
    pub async fn run() -> anyhow::Result<Self> {
        let uri = env::var("TEST_MONGODB_URI")
            .context("TEST_MONGODB_URI environment variable not set")?;

        let mut client_options = ClientOptions::parse(uri)
            .await
            .context("Failed to parse MongoDB Atlas URI")?;

        // Set a unique database name for each test run to prevent conflicts
        let db_name = format!("test_db_{}", Uuid::new_v4());
        client_options.default_database = Some(db_name.clone());

        let client = MongoDBClient::with_options(client_options)
            .context("Failed to create MongoDB client")?;

        // Verify we can connect
        client
            .list_database_names()
            .await
            .context("Failed to connect to MongoDB Atlas")?;

        Ok(Self { client, db_name })
    }

    pub fn client(&self) -> &MongoDBClient {
        &self.client
    }

    pub fn db_name(&self) -> &String {
        &self.db_name
    }

    pub fn database(&self) -> Database {
        self.client.database(&self.db_name)
    }

    /// Cleans up all collections in the test database
    pub async fn cleanup_collections(&self) -> anyhow::Result<()> {
        let db = self.database();

        // Get all collections in the database
        let collections = db.list_collection_names().await?;

        // Drop each collection
        for collection in collections {
            db.collection::<bson::Document>(&collection)
                .drop()
                .await
                .with_context(|| format!("Failed to drop collection {}", collection))?;
        }

        Ok(())
    }

    pub async fn cleanup(&self) -> anyhow::Result<()> {
        // Drop the entire test database
        self.client
            .database(&self.db_name)
            .drop()
            .await
            .with_context(|| format!("Failed to drop test database {}", self.db_name))
    }
}
