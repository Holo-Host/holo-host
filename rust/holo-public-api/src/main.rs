use actix_web::{middleware::from_fn, web, App, HttpServer};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

mod controllers;
mod middleware;

#[allow(dead_code)]
mod providers;

#[cfg(test)]
#[allow(dead_code)]
mod tests;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // setup tracing
    tracing_subscriber::fmt::init();

    // load config
    let app_config = providers::config::load_config().unwrap_or_else(|err| {
        tracing::error!("Error loading config: {}", err);
        std::process::exit(1);
    });

    // setup docs
    let mut docs = controllers::setup_docs();
    docs.components.as_mut().unwrap().security_schemes.insert(
        "Bearer".to_string(),
        SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization")))
    );
    docs.info.title = "Holo Public API".to_string();
    docs.info.version = "1.0.0".to_string();
    docs.servers = Some(vec![
        utoipa::openapi::Server::new(app_config.host.clone())
    ]);

    // setup database
    let db = providers::database::setup_database(
        &app_config.database_url, "holo"
    ).await.unwrap_or_else(|err| {
        tracing::error!("Error setting up database: {}", err);
        std::process::exit(1);
    });

    // start server
    println!("Started server on {}/swagger/", app_config.host);
    let port = app_config.port;
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_config.clone()))
            .app_data(web::Data::new(db.clone()))
            .service(
                SwaggerUi::new("/swagger/{_:.*}")
                .url("/api-docs/openapi.json", docs.clone())
            )
            .service(
                web::scope("public")
                .configure(controllers::setup_public_controllers)
            )
            .service(
                web::scope("protected")
                .wrap(from_fn(middleware::auth::auth_middleware))
                .configure(controllers::setup_private_controllers)
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
