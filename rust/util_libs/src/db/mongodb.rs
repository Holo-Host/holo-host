use crate::nats::types::ServiceError;
use anyhow::{Context, Result};
use async_trait::async_trait;
use bson::oid::ObjectId;
use bson::{self, Document};
use futures::stream::TryStreamExt;
use mongodb::options::UpdateModifications;
use mongodb::results::UpdateResult;
use mongodb::{options::IndexOptions, Client, Collection, IndexModel};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[async_trait]
pub trait MongoDbAPI<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync,
{
    type Error;

    async fn aggregate<R: for<'de> Deserialize<'de>>(
        &self,
        pipeline: Vec<Document>,
    ) -> Result<Vec<R>, Self::Error>;
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>, Self::Error>;
    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>, Self::Error>;
    async fn insert_one_into(&self, item: T) -> Result<ObjectId, Self::Error>;
    async fn update_many_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
    ) -> Result<UpdateResult, Self::Error>;
    async fn update_one_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
    ) -> Result<UpdateResult, Self::Error>;
}

pub trait IntoIndexes {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>>;
}

#[derive(Debug, Clone)]
pub struct MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
{
    pub inner: Collection<T>,
    indices: Vec<IndexModel>,
}

impl<T> MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
{
    // Initialize database and return in form of an MongoDbAPI
    // NB: Each `mongodb::Client` clone is an alias of an Arc type and allows for multiple references of the same connection pool.
    pub async fn new(
        client: &Client,
        db_name: &str,
        collection_name: &str,
    ) -> Result<Self, ServiceError> {
        let collection = client.database(db_name).collection::<T>(collection_name);
        let indices = vec![];

        Ok(MongoCollection {
            inner: collection,
            indices,
        })
    }

    pub async fn apply_indexing(&mut self) -> Result<&mut Self> {
        let schema_indices = T::default().into_indices()?;
        let mut indices = self.indices.to_owned();

        for (indexed_field, opts) in schema_indices.into_iter() {
            let options = Some(opts.unwrap_or_default());
            let index = IndexModel::builder()
                .keys(indexed_field)
                .options(options)
                .build();

            indices.push(index);
        }

        self.indices = indices.clone();

        // Apply the indices to the mongodb collection schema
        self.inner.create_indexes(indices).await?;
        Ok(self)
    }
}

#[async_trait]
impl<T> MongoDbAPI<T> for MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes + Debug,
{
    type Error = ServiceError;

    async fn aggregate<R>(&self, pipeline: Vec<Document>) -> Result<Vec<R>, Self::Error>
    where
        R: for<'de> Deserialize<'de>,
    {
        log::trace!("Aggregate pipeline {:?}", pipeline);
        let cursor = self.inner.aggregate(pipeline).await?;

        let results_doc: Vec<bson::Document> =
            cursor.try_collect().await.map_err(ServiceError::Database)?;

        let results: Vec<R> = results_doc
            .into_iter()
            .map(|doc| {
                bson::from_document::<R>(doc).with_context(|| "Failed to deserialize document")
            })
            .collect::<Result<Vec<R>>>()
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        Ok(results)
    }

    async fn get_one_from(&self, filter: Document) -> Result<Option<T>, Self::Error> {
        log::trace!("Get_one_from filter {:?}", filter);

        let item = self
            .inner
            .find_one(filter)
            .await
            .map_err(ServiceError::Database)?;

        log::debug!("get_one_from item {:?}", item);
        Ok(item)
    }

    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>, Self::Error> {
        let cursor = self.inner.find(filter).await?;
        let results: Vec<T> = cursor.try_collect().await.map_err(ServiceError::Database)?;
        Ok(results)
    }

    async fn insert_one_into(&self, item: T) -> Result<ObjectId, Self::Error> {
        let result = self
            .inner
            .insert_one(item)
            .await
            .map_err(ServiceError::Database)?;

        let mongo_id = result
            .inserted_id
            .as_object_id()
            .ok_or(ServiceError::Internal(format!(
                "Failed to read the insert id after inserting item. insert_result={:?}.",
                result
            )))?;

        Ok(mongo_id)
    }

    async fn update_many_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
    ) -> Result<UpdateResult, Self::Error> {
        self.inner
            .update_many(query, updated_doc)
            .await
            .map_err(ServiceError::Database)
    }

    async fn update_one_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
    ) -> Result<UpdateResult, Self::Error> {
        self.inner
            .update_one(query, updated_doc)
            .await
            .map_err(ServiceError::Database)
    }
}

// Helpers:
pub fn get_mongodb_url() -> String {
    std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}

#[cfg(not(target_arch = "aarch64"))]
#[cfg(test)]
mod tests {

    /// This module implements running ephemeral Mongod instances.
    /// It disables TCP and relies only unix domain sockets.
    mod mongo_runner {
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
                println!(
                    "MongoDB Server is running at {:?}",
                    new_self.socket_pathbuf()
                );

                Ok(new_self)
            }

            fn socket_pathbuf(&self) -> anyhow::Result<PathBuf> {
                Ok(PathBuf::from_str(&Self::socket_path(&self.tempdir)?)?)
            }

            pub fn client(&self) -> anyhow::Result<Client> {
                let server_address = mongodb::options::ServerAddress::Unix {
                    path: self.socket_pathbuf()?,
                };
                let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
                Ok(Client::with_options(client_options)?)
            }
        }
    }

    use super::*;
    use crate::db::schemas::{self, Capacity, Metadata};
    use bson::{self, doc, oid, DateTime};
    use dotenv::dotenv;

    #[tokio::test]
    async fn test_indexing_and_api() -> Result<()> {
        dotenv().ok();
        env_logger::init();

        let mongod = mongo_runner::MongodRunner::run().unwrap();
        let client = mongod.client().unwrap();

        let database_name = "holo-hosting-test";
        let collection_name = "host";
        let mut host_api =
            MongoCollection::<schemas::Host>::new(&client, database_name, collection_name).await?;

        // set index
        host_api.apply_indexing().await?;

        fn get_mock_host() -> schemas::Host {
            schemas::Host {
                _id: Some(oid::ObjectId::new()),
                metadata: Metadata {
                    is_deleted: false,
                    created_at: Some(DateTime::now()),
                    updated_at: Some(DateTime::now()),
                    deleted_at: None,
                },
                device_id: "placeholder_pubkey_host".to_string(),
                ip_address: "127.0.0.1".to_string(),
                remaining_capacity: Capacity {
                    memory: 16,
                    disk: 200,
                    cores: 16,
                },
                avg_uptime: 95,
                avg_network_speed: 500,
                avg_latency: 10,
                assigned_workloads: vec![oid::ObjectId::new()],
                assigned_hoster: oid::ObjectId::new(),
            }
        }

        // insert a document
        let host_0 = get_mock_host();
        let r = host_api.insert_one_into(host_0.clone()).await?;
        println!("result : {:?}", r);

        // get one (the same) document
        println!("host_0._id.unwrap() : {:?}", host_0._id.unwrap());
        let filter_one = doc! { "_id":  host_0._id.unwrap() };
        let fetched_host = host_api.get_one_from(filter_one.clone()).await?;
        let mongo_db_host = fetched_host.expect("Failed to fetch host");
        assert_eq!(mongo_db_host._id, host_0._id);

        // insert many documents
        let host_1 = get_mock_host();
        let host_2 = get_mock_host();
        let host_3 = get_mock_host();
        host_api.insert_one_into(host_1.clone()).await?;
        host_api.insert_one_into(host_2.clone()).await?;
        host_api.insert_one_into(host_3.clone()).await?;

        // get many docs
        let ids = vec![
            host_1._id.unwrap(),
            host_2._id.unwrap(),
            host_3._id.unwrap(),
        ];
        let filter_many = doc! {
            "_id": { "$in": ids.clone() }
        };
        let fetched_hosts = host_api.get_many_from(filter_many.clone()).await?;

        assert_eq!(fetched_hosts.len(), 3);
        let updated_ids: Vec<oid::ObjectId> = fetched_hosts
            .into_iter()
            .map(|h| h._id.unwrap_or_default())
            .collect();
        assert!(updated_ids.contains(&ids[0]));
        assert!(updated_ids.contains(&ids[1]));
        assert!(updated_ids.contains(&ids[2]));

        // Delete collection and all documents therein.
        let _ = host_api.inner.drop();

        Ok(())
    }
}
