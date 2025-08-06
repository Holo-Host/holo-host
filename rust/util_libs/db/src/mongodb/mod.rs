pub mod api;
pub mod collection;
pub mod traits;

/// Returns the MongoDB connection URL from environment variables.
///
/// # Returns
///
/// - If `MONGO_URI` environment variable is set, returns its value
/// - Otherwise, returns the default local MongoDB URL: "mongodb://127.0.0.1:27017"
pub fn get_mongodb_url() -> String {
    let url: Result<String, Box<dyn std::error::Error>> = (|| {
        let username = std::env::var("MONGODB_USERNAME")?;

        let cluster_file = std::env::var("MONGODB_CLUSTER_ID_FILE")?;
        let cluster_id = std::fs::read_to_string(cluster_file)?.trim().to_owned();

        let pw_file = std::env::var("MONGODB_PASSWORD_FILE")?;
        let pw = std::fs::read_to_string(pw_file)?.trim().to_owned();

        Ok(format!(
            "mongodb+srv://{}:{}@allograph-dev-mongodb-{}.mongo.ondigitalocean.com/?retryWrites=true&w=majority&authSource=admin",
            username, pw, cluster_id
        ))
    })();

    url.unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}
