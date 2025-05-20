use crate::providers::crud;
use db_utils::schemas::user::{User, USER_COLLECTION_NAME};

/// This function is used to get the refresh token version
/// if it cannot locate the refresh token version, it returns 0
pub async fn get_refresh_token_version(db: mongodb::Client, user_id: String) -> i32 {
    let user = match crud::get::<User>(db, USER_COLLECTION_NAME.to_string(), user_id.clone()).await
    {
        Ok(user) => user,
        Err(_err) => {
            return 0;
        }
    };
    if user.is_none() {
        return 0;
    }
    let user = user.unwrap();
    user.refresh_token_version
}
