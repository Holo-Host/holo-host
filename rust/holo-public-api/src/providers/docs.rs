use utoipa::openapi::security::SecurityScheme;

pub fn build_open_api_spec(
    mut docs: utoipa::openapi::OpenApi,
    server: String,
) -> utoipa::openapi::OpenApi {
    docs.components.as_mut().unwrap().security_schemes.insert(
        "Bearer".to_string(),
        SecurityScheme::Http(utoipa::openapi::security::Http::new(
            utoipa::openapi::security::HttpAuthScheme::Bearer,
        )),
    );
    docs.info.title = "Holo Public API".to_string();
    docs.info.version = "0.5.3".to_string();
    docs.info.description = Some(
        "Holo Public API is a RESTful API that allows you to interact with the Holo platform.
You can manage your API keys, workloads, blobs, user accounts, and more.
##### Rate Limiting
By default, every endpoint has a rate limit of 100 requests per minute. Some critical endpoints may have an additional rate limit.
"
        .to_string(),
    );
    // auth tag
    let mut auth_tag = utoipa::openapi::Tag::new("Auth");
    auth_tag.description = Some(
"To use this API you must authenticate with it using one of the flows provided.
After you go through an authentication flow (`login_with_apikey` etc.);
You will receive two JWT tokens (access and refresh).

## Access Token
Use the access token in the Authorization header of your request as a Bearer token.
The access token is valid for 5 minutes. Once your access token expires, you can
refresh it using the `refresh` endpoint. This endpoint will return a new access token for you.
Access tokens contain `user_id` and `permissions` as claims, signed by the server.

## Refresh Token
Refresh tokens are long-lived tokens that can be used to obtain a new access token.
A user can invalidate previous refresh tokens they have created but a refresh token
can last indefinitely until the user invalidates it or the user account is deleted.
However, by default refresh tokens are valid for 30 days.

## Permissions
- Resource - the resource you want to access (e.g. `api_key`, `user`, `workload`, `blob`).
- Actions - the action you want to perform on the resource (e.g. `Create`, `Read`, `Update`, `Delete`).
- Owner - the owner of the resource (e.g. `self`, `user_id`)."
.to_string());
    // apikey tag
    let mut apikey_tag = utoipa::openapi::Tag::new("Apikey");
    apikey_tag.description = Some(
        "Use these endpoints to manage your api keys and their permissions.

Once you login using an api key, it will let you create a refresh token that expires 
when your api key expires. If you remove or revoke your api key, 
your refresh token will be invalidated.
"
        .to_string(),
    );
    // blob tag
    let mut blob_tag = utoipa::openapi::Tag::new("Blob");
    blob_tag.description = Some(
        "These endpoints allows you to upload a blob, mainly used for deploying happs.".to_string(),
    );
    // workload tag
    let mut workload_tag = utoipa::openapi::Tag::new("Workload");
    workload_tag.description =
        Some("These endpoints allows you to manage your workloads.".to_string());
    docs.tags = Some(vec![auth_tag, apikey_tag, blob_tag, workload_tag]);
    docs.servers = Some(vec![utoipa::openapi::Server::new(server)]);
    docs
}
