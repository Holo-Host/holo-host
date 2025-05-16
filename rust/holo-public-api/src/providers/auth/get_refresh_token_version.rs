use bson::{doc, oid::ObjectId};
use db_utils::{
    mongodb::{api::MongoDbAPI, collection::MongoCollection},
    schemas::user::{User, USER_COLLECTION_NAME},
};

/// This function is used to get the refresh token version
/// if it cannot locate the refresh token version, it returns 0
pub async fn get_refresh_token_version(db: &mongodb::Client, user_id: String) -> i32 {
    let collection = match MongoCollection::<User>::new(db, "holo", USER_COLLECTION_NAME).await {
        Ok(collection) => collection,
        Err(_err) => {
            return 0;
        }
    };
    let oid = match ObjectId::parse_str(user_id) {
        Ok(oid) => oid,
        Err(_err) => {
            return 0;
        }
    };
    let doc = match collection
        .get_one_from(doc! { "_id": oid, "metadata.is_deleted": false })
        .await
    {
        Ok(doc) => doc,
        Err(_err) => {
            return 0;
        }
    };
    if doc.is_none() {
        return 0;
    }
    let doc = doc.unwrap();
    doc.refresh_token_version
}
