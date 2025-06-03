use async_trait::async_trait;
use bson::{oid::ObjectId, DateTime, Document};
use futures::stream::TryStreamExt;
use mongodb::{options::UpdateModifications, results::UpdateResult};
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};

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

    /// Deletes a single document matching the query criteria from the collection.
    ///
    /// # Arguments
    ///
    /// * `query` - Query filter as a BSON document
    ///
    async fn delete_one_from(&self, query: Document) -> Result<(), Self::Error>;
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

    async fn delete_one_from(&self, query: Document) -> Result<(), Self::Error> {
        log::debug!("Deleting document with query: {:?}", query);
        let result = self
            .inner
            .delete_one(query.clone())
            .await
            .map_err(|e| Self::handle_db_error("delete_one_from", e))?;

        log::info!("Deleted document (deleted count: {})", result.deleted_count);
        Ok(())
    }
}
