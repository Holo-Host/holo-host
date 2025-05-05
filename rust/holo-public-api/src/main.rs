use actix_web::{middleware::from_fn, web, App, HttpServer};
use utoipa::openapi::security::SecurityScheme;
use utoipa_redoc::Servable as Redoc;
use utoipa_scalar::Servable as Scalar;

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
    tracing_subscriber::fmt().init();

    // load config
    let app_config = providers::config::load_config().unwrap_or_else(|error| {
        tracing::error!("{:?}", error);
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
    docs.info.title = "Holo Public API".to_string();
    docs.info.version = "0.5.3".to_string();
    docs.tags = Some(vec![utoipa::openapi::Tag::new("Auth")]);
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
                web::get().to(|| async { web::Redirect::to("/scalar") }),
            );
            app = app.service(utoipa_redoc::Redoc::with_url("/redoc", docs.clone()));
            app = app.service(utoipa_scalar::Scalar::with_url("/scalar", docs.clone()));
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
