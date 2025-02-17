use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use mongodb::Client;
use std::env;
use utoipa::{openapi::Server, OpenApi};
use utoipa_swagger_ui::{SwaggerUi, Url};

mod example;

#[derive(OpenApi)]
#[openapi(paths(example::hello))]
struct ApiDoc;

#[get("/api-docs/json")]
async fn docs() -> impl Responder {
    let mut docs = ApiDoc::openapi();
    docs.servers = Some(vec![Server::new("http://127.0.0.1:3000")]);
    HttpResponse::Ok()
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .insert_header(("Content-Type", "Application/json"))
        .body(docs.to_pretty_json().unwrap())
}

#[allow(dead_code)]
async fn db() -> mongodb::error::Result<()> {
    let connection_uri = env::var("DB_CONNECTION_STRING").unwrap();
    let client = Client::with_uri_str(connection_uri).await?;
    let _database = client.database("orchestrator");

    Ok(())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = db().await;

    println!("Starting Server on port 3000");
    HttpServer::new(|| {
        App::new()
            .service(
                SwaggerUi::new("/api-docs/ui/{_:.*}")
                    .url(Url::new("api1", "/api-docs/json"), ApiDoc::openapi()),
            )
            .service(example::hello)
            .service(docs)
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await
}
