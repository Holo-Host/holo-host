# Holo Public Api Developer Docs

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) `cargo 1.85.0`
- [Docker](https://docs.docker.com/desktop/setup/install/linux/) `Docker version 27.5.1`

### Setup Local Development

1. Start local mongodb database

```bash
docker compose up -d
```

2. Run server

```bash
cargo run -p holo-public-api
```

### Run tests

To run tests a local mongodb database is required.
The script will start a local mongodb database using docker compose and then run the tests.

```bash
cargo test -p holo-public-api
```

## Structure

This project follows functional programming and therefor most of the project comprises of functions that call each other. All of the data is passed between the function and the api is stateless.

### Controllers

Controllers are functions that map to an endpoint exposed by the api. These functions should have openapi docs inside the code. Below is an example of a valid controller

```rs
// this will be used by utoipa to build the openapi spec
#[derive(OpenApi)]
#[openapi(paths(health_check), components(schemas(HealthCheckResponse)))]
pub struct OpenApiSpec;

// struct in which the response will be sent
#[derive(Serialize, ToSchema)]
pub struct HealthCheckResponse {
    pub status: String,
}

// the controller along with openapi docs using utoipa
#[utoipa::path(
    get,
    path = "/public/v1/general/health-check",
    tag = "General",
    summary = "Health check",
    description = "Publicly accessible health check endpoint",
    responses(
        (status = 200, body = HealthCheckResponse)
    )
)]
#[get("/v1/general/health-check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(HealthCheckResponse {
        status: "ok".to_string(),
    })
}
```

### Middleware

Middlewares are functions that can be triggered before the request is passed to a controller. These functions can be used for logging requests as they come in, authorizing requests before they hit a controller etc.

Here is an example of a simple middleware that logs a message when a new request is received by the api. These middlewares are setup in the `main.rs`.

```rs
pub async fn auth_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    print!("new incoming request");
    let resp = next.call(req).await?;
    Ok(resp.map_into_boxed_body())
}
```

### Providers

Providers are used for shared code across multiple controllers and to help make controllers more maintainable. You should follow these rules when creating providers.

#### Providers cannot return HTTP response

This is because providers may be used with multiple controllers and each controller should be responsible for returning HTTP status codes.

#### Duplicate Code in controllers

If there is duplicate code in multiple controllers then it is a good idea to make a new provider that both controller can use instead.

#### More maintainable controller

If your controller is very big and ugly, making it less maintainable in the future then you can move some of the code to a provider.

### Scheduler

Schedulers are functions that automatically trigger on a cron job.
