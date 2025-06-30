use actix_web::{middleware::from_fn, web, App, HttpServer};
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
    let host = app_config
        .clone()
        .host
        .unwrap_or("http://localhost:3000".to_string());
    let docs = providers::docs::build_open_api_spec(controllers::setup_docs(false), host.clone());
    let docs_internal =
        providers::docs::build_open_api_spec(controllers::setup_docs(true), host.clone());

    // setup database
    let mongodb_url = app_config
        .mongo_url
        .clone()
        .unwrap_or("mongodb://admin:password@localhost:27017".to_string());
    let mongodb_client = mongodb::Client::with_uri_str(mongodb_url)
        .await
        .expect("Failed to connect to MongoDB");

    // setup cache
    let valkey_url = app_config
        .redis_url
        .clone()
        .unwrap_or("redis://localhost:6379".to_string());
    let cache_pool = deadpool_redis::Config::from_url(valkey_url)
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("failed to create redis pool");

    // setup scheduler
    if app_config.enable_scheduler.unwrap_or(false) {
        scheduler::setup_scheduler(
            app_config.clone(),
            mongodb_client.clone(),
            cache_pool.clone(),
        )
        .await
        .unwrap();
    }

    // start server
    println!("Started server on {}", host.clone());
    let port = app_config.port.unwrap_or(3000);
    HttpServer::new(move || {
        // create app with required app data
        let mut app = App::new()
            .app_data(web::Data::new(app_config.clone()))
            .app_data(web::Data::new(mongodb_client.clone()))
            .app_data(web::Data::new(cache_pool.clone()))
            .wrap(from_fn(middlewares::logging::logging_middleware))
            .wrap(from_fn(middlewares::limiter::rate_limiter_middleware));

        // open api spec and docs
        app = app.route(
            "/",
            web::get().to(|| async { web::Redirect::to("/scalar") }),
        );
        app = app.service(utoipa_scalar::Scalar::with_url("/scalar", docs.clone()));
        if app_config.enable_internal_docs.unwrap_or(false) {
            app = app.service(utoipa_scalar::Scalar::with_url(
                "/scalar-internal",
                docs_internal.clone(),
            ));
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
