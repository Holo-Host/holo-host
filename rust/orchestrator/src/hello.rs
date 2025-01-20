use actix_web::{get, HttpResponse, Responder};

#[utoipa::path(
    get,
    path = "/test",
    responses(
        (status = 200, description = "Returns hello world")
    ),
    description = "test"
)]
#[get("/test")]
async fn hello() -> impl Responder {
    HttpResponse::Ok()
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .insert_header(("Content-Type", "Application/text"))
        .body("Hello world.")
}
