use anyhow::{Context, Result};
use async_trait::async_trait;
use bson::oid::ObjectId;
use bson::{self, Document};
use futures::stream::TryStreamExt;
use mongodb::options::UpdateModifications;
use mongodb::results::UpdateResult;
use mongodb::{options::IndexOptions, Client, Collection, IndexModel};
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// Helper:
pub fn get_mongodb_url() -> String {
    std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}

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
        log::trace!("Aggregate pipeline {pipeline:?}");
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
        log::trace!("Get_one_from filter {filter:?}");

        let item = self
            .inner
            .find_one(filter)
            .await
            .map_err(ServiceError::Database)?;

        log::debug!("get_one_from item {item:?}");
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
                "Failed to read the insert id after inserting item. insert_result={result:?}."
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
