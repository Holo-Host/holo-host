pub mod schemas;

use std::collections::HashMap;

use bson::Document;
use serde::de::DeserializeOwned;
use mongodb::{
    options::{
        ClientOptions,
        ResolverConfig
    },
    Client,
    Cursor,
    Database
};
use tokio::sync::OnceCell;
static INIT: OnceCell<()> = OnceCell::const_new();

async fn setup_collections(database: &Database) -> Result<(), anyhow::Error> {
    // setup validators
    let mut validators = HashMap::new();
    validators.insert(schemas::workload::WORKLOAD_COLLECTION_NAME, schemas::workload::workload_validator());
    validators.insert(schemas::api_key::API_KEY_COLLECTION_NAME, schemas::api_key::api_key_validator());
    validators.insert(schemas::host::HOST_COLLECTION_NAME, schemas::host::host_validator());
    validators.insert(schemas::user_info::USER_INFO_COLLECTION_NAME, schemas::user_info::user_info_validator());
    validators.insert(schemas::user::USER_COLLECTION_NAME, schemas::user::user_validator());

    let collections_list = database.list_collection_names(None).await?;
    for (collection_name, validator) in validators {
        // create collection if it doesn't exist
        if !collections_list.contains(&collection_name.to_string()) {
            database.create_collection(collection_name, None).await?;
        }

        // setup validator
        database.run_command(
            bson::doc!{
                "collMod": collection_name,
                "validationLevel": "strict",
                "validationAction": "error",
                "validator": bson::doc!{
                    "$jsonSchema": validator
                }
            },
            None
        ).await?;
    }

    // setup indexes
    schemas::workload::setup_workload_indexes(database).await?;
    schemas::api_key::setup_api_key_indexes(database).await?;
    schemas::host::setup_host_indexes(database).await?;
    schemas::user_info::setup_user_info_indexes(database).await?;
    
    Ok(())
}

// setup database
pub async fn setup_database(
    database_url: &str,
    database_name: &str
) -> Result<Database, anyhow::Error> {
    let options = match ClientOptions::parse_with_resolver_config(
        database_url,
        ResolverConfig::cloudflare()
    ).await {
        Ok(options) => options,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    let client = match Client::with_options(options) {
        Ok(client) => client,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    let db = client.database(database_name);

    INIT.get_or_init(|| async {
        setup_collections(&db).await.unwrap();
    }).await;

    Ok(db)
}

// converts mongodb cursor to vec array of type T
pub async fn cursor_to_vec<T: DeserializeOwned>(
    mut cursor: Cursor<Document>
) -> Result<Vec<T>, anyhow::Error> {
    let mut results = Vec::new();
    while cursor.advance().await.map_err(|e| anyhow::anyhow!(e))? {
        // `deserialize_current()` extracts the current document into your target type.
        let doc = match cursor.deserialize_current() {
            Ok(doc) => doc,
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        let converted_doc = match bson::from_document::<T>(doc) {
            Ok(converted_doc) => converted_doc,
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        results.push(converted_doc);
    }
    Ok(results)
}
