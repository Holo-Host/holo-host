use bson::oid::ObjectId;
use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use mongodb::{options::UpdateModifications, results::UpdateResult};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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
