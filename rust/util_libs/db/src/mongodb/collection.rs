use bson::{doc, Bson, DateTime, Document};
use mongodb::{options::UpdateModifications, Client, Collection, IndexModel};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use anyhow::Result;

use super::traits::{IntoIndexes, MutMetadata};

use nats_utils::types::ServiceError;// todo: remove this


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
    ) -> Result<Self, ServiceError> { // todo: remove nats service error from mongodb lib
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
    pub fn add_metadata_update(
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
    pub fn handle_db_error(operation: &str, error: mongodb::error::Error) -> ServiceError {
        log::error!("MongoDB {} operation failed: {}", operation, error);
        ServiceError::database(error, None, Some(operation.to_string()))
    }

    /// Helper method to handle internal errors consistently
    pub fn handle_internal_error(operation: &str, error: impl std::fmt::Display) -> ServiceError {
        log::error!("Internal error during {}: {}", operation, error);
        ServiceError::internal(error.to_string(), Some(operation.to_string()))
    }
}
