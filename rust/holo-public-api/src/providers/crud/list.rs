use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Get all documents from the specified MongoDB collection.
/// Returns a vector of documents.
/// If the documents are marked as deleted, they will not be returned.
/// Supports filtering, sorting, limiting, and skipping.
/// If no filter is provided, all documents will be returned.
/// If no sort is provided, the documents will be sorted by `_id` in ascending order.
/// If no limit is provided, a default limit of 100 will be used.
/// If no skip is provided, a default skip of 0 will be used.
pub async fn list<T>(
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
