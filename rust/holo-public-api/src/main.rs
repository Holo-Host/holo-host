use actix_web::{middleware::from_fn, web, App, HttpServer};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

/// hack to disable tests in build bot
/// disables all tests if the NIX_STORE environment variable is set
#[cfg(test)]
#[actix_web::test]
async fn maybe_disable_tests() {
    use std::env;

    if env::var("NIX_STORE").is_ok() {
        std::process::exit(0);
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests;

mod controllers;
mod middlewares;

#[allow(dead_code)]
mod providers;
mod scheduler;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // load config
    let app_config = providers::config::load_config().unwrap_or_else(|err| {
        tracing::error!("Error loading config: {}", err);
        std::process::exit(1);
    });

    // setup docs
    let mut docs = controllers::setup_docs();
    docs.components.as_mut().unwrap().security_schemes.insert(
        "Bearer".to_string(),
        SecurityScheme::Http(utoipa::openapi::security::Http::new(
            utoipa::openapi::security::HttpAuthScheme::Bearer,
        )),
    );
    docs.components.as_mut().unwrap().security_schemes.insert(
        "ApiKey".to_string(),
        SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("API-KEY"))),
    );
    docs.info.title = "Holo Public API".to_string();
    docs.info.version = "0.0.1".to_string();
    docs.servers = Some(vec![utoipa::openapi::Server::new(app_config.host.clone())]);

    // setup database
    let mongodb_client = mongodb::Client::with_uri_str(&app_config.mongo_url)
        .await
        .expect("Failed to connect to MongoDB");

    // setup cache
    let cache_pool = deadpool_redis::Config::from_url(&app_config.redis_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("failed to create redis pool");

    // setup scheduler
    if app_config.enable_scheduler {
        scheduler::setup_scheduler(
            app_config.clone(),
            mongodb_client.clone(),
            cache_pool.clone(),
        )
        .await
        .unwrap();
    }

    // start server
    println!("Started server on {}", app_config.host);
    let port = app_config.port;
    HttpServer::new(move || {
        // create app with required app data
        let mut app = App::new()
            .app_data(web::Data::new(app_config.clone()))
            .app_data(web::Data::new(mongodb_client.clone()))
            .app_data(web::Data::new(cache_pool.clone()))
            .wrap(from_fn(middlewares::logging::logging_middleware));

        // open api spec and swagger ui
        if app_config.enable_swagger {
            app = app.route(
                "/",
                web::get().to(|| async { web::Redirect::to("/swagger/") }),
            );
            app = app.service(
                SwaggerUi::new("/swagger/{_:.*}").url("/api-docs/openapi.json", docs.clone()),
            );
        }

        // public routes
        app = app.service(web::scope("public").configure(controllers::setup_public_controllers));

        // protected routes
        app = app.service(
            web::scope("protected")
                .wrap(from_fn(middlewares::auth::auth_middleware))
                .configure(controllers::setup_private_controllers),
        );

        app.wrap(actix_cors::Cors::permissive())
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
