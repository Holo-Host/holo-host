use bson::oid::ObjectId;
use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use mongodb::{options::UpdateModifications, results::UpdateResult};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Create a new document in the specified MongoDB collection.
/// Returns the ObjectId of the created document.
pub async fn create<T>(
    db: mongodb::Client,
    collection_name: String,
    item: T,
) -> Result<ObjectId, anyhow::Error>
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
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    let result = match collection.insert_one_into(item).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to insert document: {}", e);
            return Err(anyhow::anyhow!("Failed to insert document"));
        }
    };
    Ok(result)
}

/// Get a document by its ID from the specified MongoDB collection.
/// Returns the document if found, otherwise returns None.
/// If the document is marked as deleted, it will not be returned.
pub async fn get<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
) -> Result<Option<T>, anyhow::Error>
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
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    let result = match collection
        .get_one_from(bson::doc! { "_id": id, "metadata.is_deleted": false })
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to get document by ID: {}", e);
            return Err(anyhow::anyhow!("Failed to get document by ID"));
        }
    };
    Ok(result)
}

/// Get all documents from the specified MongoDB collection.
/// Returns a vector of documents.
/// If the documents are marked as deleted, they will not be returned.
/// Supports filtering, sorting, limiting, and skipping.
/// If no filter is provided, all documents will be returned.
/// If no sort is provided, the documents will be sorted by `_id` in ascending order.
/// If no limit is provided, a default limit of 100 will be used.
/// If no skip is provided, a default skip of 0 will be used.
pub async fn get_many<T>(
    db: mongodb::Client,
    collection_name: String,
    filter: Option<bson::Document>,
    sort: Option<bson::Document>,
    limit: Option<i64>,
    skip: Option<i64>,
) -> Result<Vec<T>, anyhow::Error>
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
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    let result = match collection
        .aggregate(vec![
            bson::doc! { "$match": { "metadata.is_deleted": false } },
            bson::doc! { "$match": filter.unwrap_or_default() },
            bson::doc! { "$sort": sort.unwrap_or(bson::doc! { "_id": 1 }) },
            bson::doc! { "$skip": skip.unwrap_or(0) },
            bson::doc! { "$limit": limit.unwrap_or(100) },
        ])
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to get all documents: {}", e);
            return Err(anyhow::anyhow!("Failed to get all documents"));
        }
    };
    Ok(result)
}

/// Update a document by its ID in the specified MongoDB collection.
/// Returns the result of the update operation.
/// If the document is marked as deleted, it will not be updated.
pub async fn update<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
    updated_item: T,
) -> Result<UpdateResult, anyhow::Error>
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
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    let updated_item_doc = match bson::to_document(&updated_item) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::error!("Failed to convert item to BSON document: {}", e);
            return Err(anyhow::anyhow!("Failed to convert item to BSON document"));
        }
    };
    let result = match collection
        .update_one_within(
            bson::doc! { "_id": id, "metadata.is_deleted": false },
            UpdateModifications::Document(bson::doc! { "$set": updated_item_doc }),
            false,
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to update document: {}", e);
            return Err(anyhow::anyhow!("Failed to update document"));
        }
    };
    Ok(result)
}

/// Soft delete a document by its ID in the specified MongoDB collection.
/// Returns the result of the update operation.
pub async fn delete<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
) -> Result<UpdateResult, anyhow::Error>
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
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    let result = match collection
        .update_one_within(
            bson::doc! { "_id": id },
            UpdateModifications::Document(bson::doc! { "$set": { "metadata.is_deleted": true } }),
            true,
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to delete document: {}", e);
            return Err(anyhow::anyhow!("Failed to delete document"));
        }
    };
    Ok(result)
}

/// Hard delete a document by its ID in the specified MongoDB collection.
/// Returns an empty result if successful.
pub async fn delete_hard<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
) -> Result<(), anyhow::Error>
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
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    match collection.delete_one_from(bson::doc! { "_id": id }).await {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::error!("Failed to delete document: {}", e);
            Err(anyhow::anyhow!("Failed to delete document"))
        }
    }
}
