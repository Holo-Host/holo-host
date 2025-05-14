use bson::oid::ObjectId;
use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use mongodb::{options::UpdateModifications, results::UpdateResult};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, str::FromStr};

#[derive(Serialize, Deserialize, Debug)]
pub struct Count {
    pub count: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Owner {
    pub owner: ObjectId,
}

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
    let oid = match ObjectId::parse_str(id) {
        Ok(oid) => oid,
        Err(error) => {
            tracing::error!("{:?}", error);
            return Err(anyhow::anyhow!("Failed to parse mongodb id"));
        }
    };
    let result = match collection
        .get_one_from(bson::doc! { "_id": oid, "metadata.is_deleted": false })
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

pub async fn get_owner<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
) -> Result<Option<String>, anyhow::Error>
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
    let oid = match ObjectId::parse_str(id) {
        Ok(oid) => oid,
        Err(_) => {
            return Err(anyhow::anyhow!("Failed to parse mongodb id"));
        }
    };
    let result = match collection
        .aggregate::<Owner>(vec![
            bson::doc! { "$match": { "_id": oid, "metadata.is_deleted": false } },
            bson::doc! { "$project": { "owner": 1 } },
        ])
        .await
    {
        Ok(result) => result,
        Err(_) => {
            return Err(anyhow::anyhow!("Failed to get owner"));
        }
    };
    if result.is_empty() {
        return Ok(None);
    }
    Ok(Some(result[0].owner.to_hex()))
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
    limit: Option<i32>,
    skip: Option<i32>,
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

/// Get all documents from the specified MongoDB collection.
/// Returns a vector of documents.
/// If the documents are marked as deleted, they will not be returned.
/// Supports filtering, sorting, limiting, and skipping.
/// If no filter is provided, all documents will be returned.
/// If no sort is provided, the documents will be sorted by `_id` in ascending order.
/// If no limit is provided, a default limit of 100 will be used.
/// If no skip is provided, a default skip of 0 will be used.
pub async fn count<T>(
    db: mongodb::Client,
    collection_name: String,
    filter: Option<bson::Document>,
) -> Result<i32, anyhow::Error>
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
        .aggregate::<Count>(vec![
            bson::doc! { "$match": { "metadata.is_deleted": false } },
            bson::doc! { "$match": filter.unwrap_or_default() },
            bson::doc! { "$count": "count" },
        ])
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to get all documents: {}", e);
            return Err(anyhow::anyhow!("Failed to get all documents"));
        }
    };
    if result.is_empty() {
        return Ok(0);
    }
    let count = result[0].count;
    Ok(count)
}

/// Update a document by its ID in the specified MongoDB collection.
/// Returns the result of the update operation.
/// If the document is marked as deleted, it will not be updated.
pub async fn update<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
    updates: bson::Document,
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
    let oid = match ObjectId::from_str(&id) {
        Ok(oid) => oid,
        Err(error) => {
            tracing::error!("{:?}", error);
            return Err(anyhow::anyhow!("Failed to parse object id"));
        }
    };
    let result = match collection
        .update_one_within(
            bson::doc! { "_id": oid, "metadata.is_deleted": false },
            UpdateModifications::Document(bson::doc! { "$set": updates }),
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
    let oid = match ObjectId::parse_str(id) {
        Ok(oid) => oid,
        Err(error) => {
            tracing::error!("{:?}", error);
            return Err(anyhow::anyhow!("failed to parse object id"));
        }
    };
    let result = match collection
        .update_one_within(
            bson::doc! { "_id": oid },
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
