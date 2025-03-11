/// MongoDB interface module providing a high-level API for MongoDB operations.
///
/// This module provides traits and implementations for interacting with MongoDB collections
/// in a type-safe manner. It includes support for common database operations, indexing,
/// and error handling integrated with the service architecture.
///
/// # Examples
///
/// ```rust,no_run
/// use anyhow::Result;
/// use mongodb::Client;
/// use db_utils::mongodb::MongoCollection;
/// use db_utils::schemas::{Host, DATABASE_NAME};
///
/// async fn example() -> Result<()> {
///     let client = Client::with_uri_str("mongodb://localhost:27017").await?;
///     let mut collection = MongoCollection::<Host>::new(&client, DATABASE_NAME, "host").await?;
///     
///     // Optionally apply indices for the collection model
///     // (Indicies defined on a schema prior to calling `MongoCollection::new(..)` are applied automatically)
///     collection.apply_indexing().await?;
///     
///     Ok(())
/// }
/// ```
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

/// Returns the MongoDB connection URL from environment variables.
///
/// # Returns
///
/// - If `MONGO_URI` environment variable is set, returns its value
/// - Otherwise, returns the default local MongoDB URL: "mongodb://127.0.0.1:27017"
pub fn get_mongodb_url() -> String {
    std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}

/// Core trait defining MongoDB operations for a collection of type `T`.
///
/// This trait provides a standardized interface for common MongoDB operations
/// including aggregation, querying, insertion, and updates.
///
/// # Type Parameters
///
/// * `T` - The type representing documents in the collection. Must be serializable,
///         deserializable, and thread-safe.
#[async_trait]
pub trait MongoDbAPI<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync,
{
    /// The error type returned by operations in this trait.
    type Error;

    /// Executes an aggregation pipeline and returns the results.
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Vector of aggregation pipeline stages as BSON documents
    ///
    /// # Returns
    ///
    /// A vector of documents of type `R` representing the aggregation results
    async fn aggregate<R: for<'de> Deserialize<'de>>(
        &self,
        pipeline: Vec<Document>,
    ) -> Result<Vec<R>, Self::Error>;

    /// Retrieves a single document matching the filter criteria.
    ///
    /// # Arguments
    ///
    /// * `filter` - Query filter as a BSON document
    ///
    /// # Returns
    ///
    /// An optional document of type `T` if found
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>, Self::Error>;

    /// Retrieves multiple documents matching the filter criteria.
    ///
    /// # Arguments
    ///
    /// * `filter` - Query filter as a BSON document
    ///
    /// # Returns
    ///
    /// A vector of documents of type `T`
    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>, Self::Error>;

    /// Inserts a single document into the collection.
    ///
    /// # Arguments
    ///
    /// * `item` - Document of type `T` to insert
    ///
    /// # Returns
    ///
    /// The ObjectId of the inserted document
    async fn insert_one_into(&self, item: T) -> Result<ObjectId, Self::Error>;

    /// Updates multiple documents matching the query criteria.
    ///
    /// # Arguments
    ///
    /// * `query` - Query filter as a BSON document
    /// * `updated_doc` - Update modifications to apply
    ///
    /// # Returns
    ///
    /// Result of the update operation
    async fn update_many_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
    ) -> Result<UpdateResult, Self::Error>;

    /// Updates a single document matching the query criteria.
    ///
    /// # Arguments
    ///
    /// * `query` - Query filter as a BSON document
    /// * `updated_doc` - Update modifications to apply
    ///
    /// # Returns
    ///
    /// Result of the update operation
    async fn update_one_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
    ) -> Result<UpdateResult, Self::Error>;
}

/// Trait for defining MongoDB indices for a collection.
///
/// Implementors of this trait can define the indices that should be created
/// for their corresponding MongoDB collection.
pub trait IntoIndexes {
    /// Converts the implementation into a vector of index definitions.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the index specification document and optional index options
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>>;
}

/// Wrapper type for MongoDB collections providing additional functionality.
///
/// This struct wraps a MongoDB collection and provides methods for managing
/// indices and implementing the `MongoDbAPI` trait.
///
/// # Type Parameters
///
/// * `T` - The type representing documents in the collection. Must implement
///         necessary traits for serialization, deserialization, and indexing.
#[derive(Debug, Clone)]
pub struct MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
{
    /// The underlying MongoDB collection
    pub inner: Collection<T>,
    /// Collection indices
    indices: Vec<IndexModel>,
}

impl<T> MongoCollection<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + Debug + IntoIndexes,
{
    /// Creates a new `MongoCollection` instance and applies the defined indices.
    ///
    /// # Arguments
    ///
    /// * `client` - MongoDB client instance
    /// * `db_name` - Name of the database
    /// * `collection_name` - Name of the collection
    ///
    /// # Returns
    ///
    /// A new `MongoCollection` instance with indices applied, wrapped in a Result
    ///
    // NB: Each `mongodb::Client` clone is an alias of an Arc type and allows for multiple references of the same connection pool.
    pub async fn new(
        client: &Client,
        db_name: &str,
        collection_name: &str,
    ) -> Result<Self, ServiceError> {
        let collection = client.database(db_name).collection::<T>(collection_name);
        let indices = vec![];

        let mut mongo_collection = MongoCollection {
            inner: collection,
            indices,
        };

        // Apply indices during initialization
        mongo_collection
            .apply_indexing()
            .await
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        Ok(mongo_collection)
    }

    /// Applies the defined indices to the MongoDB collection.
    ///
    /// This method creates the indices defined by the collection's type `T`
    /// through its implementation of `IntoIndexes`.
    ///
    /// # Returns
    ///
    /// A reference to self for method chaining
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

        if !indices.is_empty() {
            self.indices = indices.clone();
            // Apply the indices to the mongodb collection schema
            self.inner.create_indexes(indices.clone()).await?;
        }

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
