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
    std::env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}
