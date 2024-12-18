use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bson::{self, doc, Document};
use futures::stream::TryStreamExt;
use mongodb::results::DeleteResult;
use mongodb::{options::IndexOptions, Client, Collection, IndexModel};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(thiserror::Error, Debug, Clone)]
pub enum ServiceError {
    #[error("Internal Error: {0}")]
    Internal(String),
    #[error(transparent)]
    Database(#[from] mongodb::error::Error),
}

#[async_trait]
pub trait MongoDbPool<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync,
{
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>>;
    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>>;
    async fn insert_one_into(&self, item: T) -> Result<String>;
    async fn insert_many_into(&self, items: Vec<T>) -> Result<Vec<String>>;
    async fn delete_all_from(&self) -> Result<DeleteResult>;
}

pub trait IntoIndexes {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>>;
}

#[derive(Debug, Clone)]
pub struct MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
{
    collection: Collection<T>,
    indices: Vec<IndexModel>,
}

impl<T> MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
{
    // Initialize database and return in form of an MongoDbPool
    // NB: Each `mongodb::Client` clone is an alias of an Arc type and allows for multiple references of the same connection pool.
    pub async fn new(
        client: &Client,
        db_name: &str,
        collection_name: &str,
    ) -> Result<Self, ServiceError> {
        let collection = client.database(db_name).collection::<T>(collection_name);
        let indices = vec![];

        Ok(MongoCollection {
            collection,
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
        self.collection.create_indexes(indices).await?;
        Ok(self)
    }
}

#[async_trait]
impl<T> MongoDbPool<T> for MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes + Debug,
{
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>> {
        log::info!("get_one_from filter {:?}", filter);

        let item = self
            .collection
            .find_one(filter)
            .await
            .map_err(ServiceError::Database)?;

        log::info!("item {:?}", item);
        Ok(item)
    }

    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>> {
        let cursor = self.collection.find(filter).await?;
        let results: Vec<T> = cursor.try_collect().await.map_err(ServiceError::Database)?;
        Ok(results)
    }

    async fn insert_one_into(&self, item: T) -> Result<String> {
        let result = self
            .collection
            .insert_one(item)
            .await
            .map_err(ServiceError::Database)?;

        Ok(result.inserted_id.to_string())
    }

    async fn insert_many_into(&self, items: Vec<T>) -> Result<Vec<String>> {
        let result = self
            .collection
            .insert_many(items)
            .await
            .map_err(ServiceError::Database)?;

        let ids = result
            .inserted_ids
            .values()
            .map(|id| id.to_string())
            .collect();
        Ok(ids)
    }

    async fn delete_all_from(&self) -> Result<DeleteResult> {
        self.collection
            .delete_many(doc! {})
            .await
            .map_err(|e| anyhow!(e))
    }
}

// Helpers:
pub fn get_mongodb_url() -> String {
    std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schemas;
    use bson::{self, doc, oid};
    use dotenv::dotenv;
    use mongodb::{options::ClientOptions, Client as MongoDBClient};

    #[tokio::test]
    async fn test_indexing_and_api() -> Result<()> {
        dotenv().ok();
        env_logger::init();

        let mongo_uri =
            std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string());
        let client_options = ClientOptions::parse(mongo_uri).await?;
        let client = MongoDBClient::with_options(client_options)?;

        let database_name = "holo-hosting-test";
        let collection_name = "host";
        let mut host_api =
            MongoCollection::<schemas::Host>::new(&client, &database_name, collection_name).await?;

        // set index
        host_api.apply_indexing().await?;

        fn get_mock_host() -> schemas::Host {
            schemas::Host {
                _id: oid::ObjectId::new().to_string(),
                device_id: vec!["mac_id".to_string()],
                ip_address: "127.0.0.1".to_string(),
                remaining_capacity: 50,
                avg_uptime: 95,
                avg_network_speed: 500,
                avg_latency: 10,
                vms: vec![],
                assigned_workloads: "workload_id".to_string(),
                assigned_hoster: "hoster".to_string(),
            }
        }

        // insert a document
        let host_0 = get_mock_host();
        host_api.insert_one_into(host_0.clone()).await?;

        // get one (the same) document
        let filter_one = doc! { "_id":  host_0._id.to_string() };
        let fetched_host = host_api.get_one_from(filter_one.clone()).await?;
        let mongo_db_host = fetched_host.unwrap();
        assert_eq!(mongo_db_host._id, host_0._id);

        // insert many documents
        let host_1 = get_mock_host();
        let host_2 = get_mock_host();
        let host_3 = get_mock_host();
        host_api
            .insert_many_into(vec![host_1.clone(), host_2.clone(), host_3.clone()])
            .await?;

        // get many docs
        let ids = vec![
            host_1._id.to_string(),
            host_2._id.to_string(),
            host_3._id.to_string(),
        ];
        let filter_many = doc! {
            "_id": { "$in": ids }
        };
        let fetched_hosts = host_api.get_many_from(filter_many.clone()).await?;

        assert_eq!(fetched_hosts.len(), 3);
        let ids: Vec<String> = fetched_hosts.into_iter().map(|h| h._id).collect();
        assert!(ids.contains(&ids[0]));
        assert!(ids.contains(&ids[1]));
        assert!(ids.contains(&ids[2]));

        // delete all documents
        let DeleteResult { deleted_count, .. } = host_api.delete_all_from().await?;
        assert_eq!(deleted_count, 4);
        let fetched_host = host_api.get_one_from(filter_one).await?;
        let fetched_hosts = host_api.get_many_from(filter_many).await?;
        assert!(fetched_host.is_none());
        assert!(fetched_hosts.is_empty());

        Ok(())
    }
}
