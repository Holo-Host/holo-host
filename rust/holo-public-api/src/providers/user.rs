use bson::oid::ObjectId;
use db_utils::{
    mongodb::{api::MongoDbAPI, collection::MongoCollection},
    schemas::user::{User, USER_COLLECTION_NAME},
};

/// get a user using user_id
pub async fn get_user(
    db: &mongodb::Client,
    user_id: String,
) -> Result<Option<User>, anyhow::Error> {
    let collection = match MongoCollection::<User>::new(db, "holo", USER_COLLECTION_NAME).await {
        Ok(collection) => collection,
        Err(_err) => {
            return Err(anyhow::anyhow!("Failed to get MongoDB collection"));
        }
    };
    let oid = match ObjectId::parse_str(user_id) {
        Ok(oid) => oid,
        Err(_err) => {
            return Err(anyhow::anyhow!("Failed to parse object id"));
        }
    };
    let doc = match collection.get_one_from(bson::doc! { "_id": oid }).await {
        Ok(doc) => doc,
        Err(_err) => {
            return Err(anyhow::anyhow!("Failed to get MongoDB collection"));
        }
    };
    if doc.is_none() {
        return Ok(None);
    }
    let doc = doc.unwrap();
    Ok(Some(doc))
}
