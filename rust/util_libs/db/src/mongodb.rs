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
use anyhow::Result;
use async_trait::async_trait;
use bson::{self, doc, oid::ObjectId, Bson, DateTime, Document};
use futures::stream::TryStreamExt;
use mongodb::options::UpdateModifications;
use mongodb::results::UpdateResult;
use mongodb::{options::IndexOptions, Client, Collection, IndexModel};
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::schemas::metadata::Metadata;

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

    /// Retrieves a single document matching the filter criteria from collection.
    ///
    /// # Arguments
    ///
    /// * `filter` - Query filter as a BSON document
    ///
    /// # Returns
    ///
    /// An optional document of type `T` if found
    async fn get_one_from(&self, filter: Document) -> Result<Option<T>, Self::Error>;

    /// Retrieves multiple documents matching the filter criteria from collection.
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

    /// Updates multiple documents matching the query criteria in the collection.
    ///
    /// # Arguments
    ///
    /// * `query` - Query filter as a BSON document
    /// * `updated_doc` - Update modifications to apply
    /// * `should_mark_deleted` - Flag to determine whether to mark update as a "delete update".
    ///
    /// # Returns
    ///
    /// Result of the update operation
    async fn update_many_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
        should_mark_deleted: bool,
    ) -> Result<UpdateResult, Self::Error>;

    /// Updates a single document matching the query criteria in the collection.
    ///
    /// # Arguments
    ///
    /// * `query` - Query filter as a BSON document
    /// * `updated_doc` - Update modifications to apply
    /// * `should_mark_deleted` - Flag to determine whether to mark update as a "delete update".
    ///
    /// # Returns
    ///
    /// Result of the update operation
    async fn update_one_within(
        &self,
        query: Document,
        updated_doc: UpdateModifications,
        should_mark_deleted: bool,
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

pub trait MutMetadata {
    fn mut_metadata(&mut self) -> &mut Metadata;
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
    T: Serialize
        + for<'de> Deserialize<'de>
        + Unpin
        + Send
        + Sync
        + Default
        + IntoIndexes
        + MutMetadata,
{
    /// The underlying MongoDB collection
    pub inner: Collection<T>,
    /// Collection indices
    indices: Vec<IndexModel>,
}

impl<T> MongoCollection<T>
where
    T: Serialize
        + for<'de> Deserialize<'de>
        + Unpin
        + Send
        + Sync
        + Default
        + Debug
        + IntoIndexes
        + MutMetadata,
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
        log::debug!(
            "Creating new MongoDB collection: {}.{}",
            db_name,
            collection_name
        );
        let collection = client.database(db_name).collection::<T>(collection_name);
        let indices = vec![];

        let mut mongo_collection = MongoCollection {
            inner: collection,
            indices,
        };

        // Apply indices during initialization
        mongo_collection.apply_indexing().await.map_err(|e| {
            log::error!(
                "Failed to apply indices to collection {}.{}: {}",
                db_name,
                collection_name,
                e
            );
            ServiceError::internal(
                format!("Failed to apply indices: {}", e),
                Some(format!("Collection: {}.{}", db_name, collection_name)),
            )
        })?;

        log::info!(
            "Successfully created MongoDB collection {}.{}",
            db_name,
            collection_name
        );
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
        log::debug!("Applying indices to collection");
        let schema_indices = T::default().into_indices().map_err(|e| {
            log::error!("Failed to get indices from schema: {}", e);
            e
        })?;

        let mut indices = self.indices.to_owned();

        for (indexed_field, opts) in schema_indices.into_iter() {
            let options = Some(opts.unwrap_or_default());
            let index = IndexModel::builder()
                .keys(indexed_field.clone())
                .options(options.clone())
                .build();

            log::debug!(
                "Adding index: {:?} with options: {:?}",
                indexed_field,
                options
            );
            indices.push(index);
        }

        if !indices.is_empty() {
            self.indices = indices.clone();
            // Apply the indices to the mongodb collection schema
            self.inner.create_indexes(indices).await.map_err(|e| {
                log::error!("Failed to create indices: {}", e);
                e
            })?;
            log::info!(
                "Successfully applied {} indices to collection",
                self.indices.len()
            );
        } else {
            log::info!("No indices to apply for collection");
        }

        Ok(self)
    }

    /// Updates a single document to include the metadata appropriate fields.
    ///
    /// # Arguments
    ///
    /// * `updated_doc` - Update modifications to apply
    /// * `should_mark_deleted` - Flag to determine whether to mark update as a "delete update".
    ///
    /// # Returns
    ///
    /// The updated_doc (UpdateModifications) with added metadata fields
    fn add_metadata_update(
        &self,
        mut updated_doc: UpdateModifications,
        should_mark_deleted: bool,
        op_name: &str,
    ) -> Result<UpdateModifications, ServiceError> {
        let now = DateTime::now();
        let mut metadata_updates = doc! { "metadata.updated_at": now };

        if should_mark_deleted {
            metadata_updates.insert("metadata.is_deleted", true);
            metadata_updates.insert("metadata.deleted_at", Some(now));
        }

        // Helper function to insert metadata updates
        let insert_metadata_updates = |set_doc: &mut Document| {
            if let Some(metadata) = set_doc.get_mut("metadata").and_then(Bson::as_document_mut) {
                metadata.insert("updated_at", now);
                if should_mark_deleted {
                    metadata.insert("is_deleted", true);
                    metadata.insert("deleted_at", now);
                }
            } else {
                set_doc.extend(metadata_updates.clone());
            }
        };

        // Modify `updated_doc` to include metadata updates
        match &mut updated_doc {
            UpdateModifications::Document(doc) => {
                let set_doc = doc
                    .entry("$set".to_string())
                    .or_insert_with(|| Bson::Document(Document::new()));
                if let Some(set_doc) = set_doc.as_document_mut() {
                    insert_metadata_updates(set_doc);
                }
            }
            UpdateModifications::Pipeline(docs) => {
                if let Some(set_stage) = docs.iter_mut().find(|d| d.contains_key("$set")) {
                    if let Some(set_doc) = set_stage.get_mut("$set").and_then(Bson::as_document_mut)
                    {
                        insert_metadata_updates(set_doc);
                    }
                } else {
                    // Inserts as the second item, as the first is the record matching statement
                    docs.insert(1, doc! { "$set": metadata_updates });
                }
            }
            _ => {
                return Err(Self::handle_internal_error(
                    op_name,
                    "Unexpected UpdateModifications type",
                ))
            }
        }

        Ok(updated_doc)
    }

    /// Helper method to handle MongoDB errors consistently
    fn handle_db_error(operation: &str, error: mongodb::error::Error) -> ServiceError {
        log::error!("MongoDB {} operation failed: {}", operation, error);
        ServiceError::database(error, None, Some(operation.to_string()))
    }

    /// Helper method to handle internal errors consistently
    fn handle_internal_error(operation: &str, error: impl std::fmt::Display) -> ServiceError {
        log::error!("Internal error during {}: {}", operation, error);
        ServiceError::internal(error.to_string(), Some(operation.to_string()))
    }
}

#[async_trait]
impl<T> MongoDbAPI<T> for MongoCollection<T>
where
    T: Serialize
        + for<'de> Deserialize<'de>
        + Unpin
        + Send
        + Sync
        + Default
        + Debug
        + IntoIndexes
        + MutMetadata,
{
    type Error = ServiceError;

    async fn aggregate<R>(&self, pipeline: Vec<Document>) -> Result<Vec<R>, Self::Error>
    where
        R: for<'de> Deserialize<'de>,
    {
        log::debug!("Executing aggregation pipeline: {:?}", pipeline);
        let cursor = self
            .inner
            .aggregate(pipeline.clone())
            .await
            .map_err(|e| Self::handle_db_error("aggregate", e))?;

        let results_doc: Vec<bson::Document> = cursor
            .try_collect()
            .await
            .map_err(|e| Self::handle_db_error("aggregate collect", e))?;

        let results: Vec<R> = results_doc
            .into_iter()
            .map(|doc| bson::from_document::<R>(doc))
            .collect::<Result<Vec<R>, _>>()
            .map_err(|e| Self::handle_internal_error("aggregate deserialize", e))?;

        log::debug!("Aggregation returned {} results", results.len());
        Ok(results)
    }

    async fn get_one_from(&self, filter: Document) -> Result<Option<T>, Self::Error> {
        log::debug!("Getting one document with filter: {:?}", filter);
        let item = self
            .inner
            .find_one(filter.clone())
            .await
            .map_err(|e| Self::handle_db_error("get_one_from", e))?;

        if let Some(ref doc) = item {
            log::debug!("Found document: {:?}", doc);
        } else {
            log::debug!("No document found matching filter");
        }

        Ok(item)
    }

    async fn get_many_from(&self, filter: Document) -> Result<Vec<T>, Self::Error> {
        log::debug!("Getting multiple documents with filter: {:?}", filter);
        let cursor = self
            .inner
            .find(filter.clone())
            .await
            .map_err(|e| Self::handle_db_error("get_many_from", e))?;

        let results: Vec<T> = cursor
            .try_collect()
            .await
            .map_err(|e| Self::handle_db_error("get_many_from collect", e))?;

        log::debug!("Found {} documents", results.len());
        Ok(results)
    }

    async fn insert_one_into(&self, mut item: T) -> Result<ObjectId, Self::Error> {
        log::debug!("Inserting new document");

        let metadata = item.mut_metadata();
        metadata.is_deleted = false;
        metadata.created_at = Some(DateTime::now());
        metadata.updated_at = Some(DateTime::now());

        let result = self
            .inner
            .insert_one(item)
            .await
            .map_err(|e| Self::handle_db_error("insert_one_into", e))?;

        let mongo_id = result.inserted_id.as_object_id().ok_or_else(|| {
            Self::handle_internal_error("insert_one_into", "Failed to read inserted ID from result")
        })?;

        log::info!("Successfully inserted document with ID: {}", mongo_id);
        Ok(mongo_id)
    }

    async fn update_many_within(
        &self,
        query: Document,
        mut updated_doc: UpdateModifications,
        should_mark_deleted: bool,
    ) -> Result<UpdateResult, Self::Error> {
        log::debug!(
            "Updating multiple documents - Query: {:?}, Should mark deleted: {}",
            query,
            should_mark_deleted
        );

        updated_doc =
            self.add_metadata_update(updated_doc, should_mark_deleted, "update_many_within")?;

        let result = self
            .inner
            .update_many(query.clone(), updated_doc)
            .await
            .map_err(|e| Self::handle_db_error("update_many_within", e))?;

        log::info!(
            "Updated {} documents (matched: {})",
            result.modified_count,
            result.matched_count
        );
        Ok(result)
    }

    async fn update_one_within(
        &self,
        query: Document,
        mut updated_doc: UpdateModifications,
        should_mark_deleted: bool,
    ) -> Result<UpdateResult, Self::Error> {
        log::debug!(
            "Updating single document - Query: {:?}, Should mark deleted: {}",
            query,
            should_mark_deleted
        );

        updated_doc =
            self.add_metadata_update(updated_doc, should_mark_deleted, "update_one_within")?;

        let result = self
            .inner
            .update_one(query.clone(), updated_doc)
            .await
            .map_err(|e| Self::handle_db_error("update_one_within", e))?;

        log::info!(
            "Updated document (matched: {}, modified: {})",
            result.matched_count,
            result.modified_count
        );
        Ok(result)
    }
}
