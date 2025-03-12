use actix_limitation::RateLimiter;
use actix_web::{middleware::from_fn, web, App, HttpServer};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, OAuth2, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

mod controllers;
mod middleware;

#[allow(dead_code)]
mod providers;

#[allow(dead_code)]
mod scheduler;

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
        SecurityScheme::Http(utoipa::openapi::security::Http::new(
            utoipa::openapi::security::HttpAuthScheme::Bearer
        ))
    );
    docs.components.as_mut().unwrap().security_schemes.insert(
        "ApiKey".to_string(),
        SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("API-KEY")))
    );
    docs.info.title = "Holo Public API".to_string();
    docs.info.description = Some("Holo Public API has a limit of 10 requests per second for each user.".to_string());
    docs.info.version = "1.0.0".to_string();
    docs.servers = Some(vec![
        utoipa::openapi::Server::new(app_config.host.clone())
    ]);

    // setup database
    let mongodb = providers::database::setup_database(
        &app_config.mongo_url, "holo"
    ).await.unwrap_or_else(|err| {
        tracing::error!("Error setting up database: {}", err);
        std::process::exit(1);
    });

    // setup cache
    let cache = providers::cache::setup_cache(
        &app_config.redis_url
    ).await.unwrap_or_else(|err| {
        tracing::error!("Error setting up cache: {}", err);
        std::process::exit(1);
    });

    // setup scheduler
    if app_config.enable_scheduler {
        scheduler::setup_scheduler(
            app_config.clone(),
            mongodb.clone(),
            cache.clone()
        ).await.unwrap();
    }

    // setup rate limiters
    // limit requests by ip for unauthenticated users
    let limit_by_ip = providers::limiter::limit_requests_by_ip(
        &app_config.redis_url,
        10, // amount of requests
        1 // amount of seconds
    );

    // limit requests by user for authenticated users
    let limit_by_user = providers::limiter::limit_requests_by_user(
        &app_config.redis_url,
        10, // amount of requests
        1 // amount of seconds
    );

    // start server
    println!("Started server on {}", app_config.host);
    let port = app_config.port;
    HttpServer::new(move || {
        // create app with required app data
        let mut app = App::new()
            .app_data(web::Data::new(app_config.clone()))
            .app_data(web::Data::new(mongodb.clone()))
            .app_data(web::Data::new(cache.clone()))
            .wrap(from_fn(middleware::logging::logging_middleware));

        // open api spec and swagger ui
        if app_config.enable_swagger {
            app = app.route("/", web::get().to(|| async {
                web::Redirect::to("/swagger/")
            }));
            app = app.service(
                SwaggerUi::new("/swagger/{_:.*}")
                .url("/api-docs/openapi.json", docs.clone())
            );
        }

        // public routes
        app = app.service(
            web::scope("public")
            .wrap(RateLimiter::default())
            .app_data(web::Data::new(limit_by_ip.clone()))
            .configure(controllers::setup_public_controllers)
        );

        // protected routes
        app = app.service(
            web::scope("protected")
            .wrap(from_fn(middleware::auth::auth_middleware))
            .wrap(RateLimiter::default())
            .app_data(web::Data::new(limit_by_user.clone()))
            .configure(controllers::setup_private_controllers)
        );

        app
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}