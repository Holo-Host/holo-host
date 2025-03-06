# How to Contribute

## Guidelines

- This project uses functional programming standard
- All controllers should be located in the `src/controllers` directory.
- All providers should be located in the `src/providers` directory.
- All middleware should be located in the `src/middleware` directory.
- All controllers should have open api documentation with summary, description, tags and types.
- Shared code should be part of a provider.
- Integration tests should be located next to the controller inside a `tests` directory.
- Unit tests should be located next to the provider inside a `tests` directory.
- Documentation should be located in the `docs` directory.
- Providers are allowed to have dead code
- No error handling is required for integration & unit tests. You are free to use `unwrap()`.
- Avoid using `unwrap()` in controllers, middleware and providers.

## Controllers

Controllers are located in the `src/controllers` directory.
Each controller is responsible for handling a specific route and returning a response.
You can access web::Data in the controller by using `web::Data<AppConfig>` or `web::Data<Database>`.

Please follow the following rules when writing controllers:
- POST - create a new resource
- GET - read / fetch a resource
- PUT - update the full contents of a resource
- PATCH - update a subset of fields on a resource
- DELETE - delete a resource

## Providers

Providers are responsible for hidding complexity and using shared code across controllers.
ALL 3rd party dependencies should go through their own provider.

They are located in the `src/providers` directory.

## Middleware
Middleware is used when you want to do something with the request before it reaches the controller.
Middleware should only be used when your changes should be effecting multiple controllers.

It is located in the `src/middleware` directory.


## Converting to bson

To convert a struct to bson, use the `bson::to_bson` function.

```rust
bson::to_bson(&workload::Workload{}).unwrap();
```

To convert a bson document to a struct, use the `bson::from_bson` function.

```rust
bson::from_bson::<workload::Workload>(bson::to_bson(&workload::Workload{}).unwrap()).unwrap();
```