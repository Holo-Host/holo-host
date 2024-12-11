use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bson::{self, doc, Document};
use futures::stream::TryStreamExt;
use mongodb::results::DeleteResult;
use mongodb::{
    options::{ClientOptions, IndexOptions},
    Client, Collection, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt::Debug;

#[derive(thiserror::Error, Debug, Clone)]
pub enum ServiceError {
    #[error("Internal Error: {0}")]
    Internal(String),
    #[error(transparent)]
    Database(#[from] mongodb::error::Error),
}

// Note: Each mongodb::Client clone is an alias of an Arc type and allows for multiple references of the same connection pool.
#[async_trait]
pub trait MongoDbPool<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync,
{
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>>;
    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>>;
    async fn insert_many_into(&self, items: Vec<T>) -> Result<Vec<bson::oid::ObjectId>>;
    async fn delete_all_from(&self) -> Result<DeleteResult>;
}

pub trait IntoIndexes {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>>;
}

#[derive(Debug, Clone)]
pub struct MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + IntoIndexes,
{
    collection: Collection<T>,
    indices: Vec<IndexModel>,
    schema: T,
}

impl<T> MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + IntoIndexes,
{
    // Initialize database and return in form of an MongoDbPool
    pub async fn new(
        db_name: &str,
        collection_name: &str,
        schema: T,
    ) -> Result<Self, ServiceError> {
        let mongo_uri =
            std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string());

        let client_options = ClientOptions::parse(mongo_uri).await?;
        let client = Client::with_options(client_options)?;
        let collection = client.database(db_name).collection::<T>(collection_name);
        let indices = vec![];

        Ok(MongoCollection {
            collection,
            indices,
            schema,
        })
    }

    pub async fn apply_indexing(&mut self) -> Result<&mut Self> {
        let schema_indices = self.schema.into_indices()?;
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
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + IntoIndexes,
    bson::Document: Borrow<T>,
{
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>> {
        let item = self
            .collection
            .find_one(filter)
            .await
            .map_err(ServiceError::Database)?;
        Ok(item)
    }

    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>> {
        let cursor = self.collection.find(filter).await?;
        let results: Vec<T> = cursor.try_collect().await.map_err(ServiceError::Database)?;
        Ok(results)
    }

    async fn insert_many_into(&self, items: Vec<T>) -> Result<Vec<mongodb::bson::oid::ObjectId>> {
        let docs: Vec<Document> = items
            .into_iter()
            .map(|item| bson::to_document(&item).unwrap())
            .collect::<Vec<_>>();

        let result = self
            .collection
            .insert_many(docs)
            .await
            .map_err(ServiceError::Database)?;

        let ids = result
            .inserted_ids
            .values()
            .filter_map(|id| id.as_object_id())
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
