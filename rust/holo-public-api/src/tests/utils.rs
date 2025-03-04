use actix_web::{
    body::{BoxBody, MessageBody}, dev::{HttpServiceFactory, ServiceRequest, ServiceResponse}, http::{
        header::HeaderMap,
        StatusCode
    }, middleware::Next, test::{
        self, TestRequest
    }, web, App, Error, HttpMessage
};
use mongodb::Database;
use crate::providers::{
    config::AppConfig,
    database, jwt::{
        sign_access_token,
        sign_refresh_token,
        AccessTokenClaims,
        RefreshTokenClaims
    }
};

#[derive(Debug, Clone)]
pub struct WebData {
    pub config: Option<AppConfig>,
    pub db: Option<Database>,
    pub auth: Option<AccessTokenClaims>
}

pub struct IntegrationTestResponse {
    pub status: StatusCode,
    pub body: Option<bson::Document>,
    pub headers: HeaderMap,
}

pub async fn auth_middleware(
    req: ServiceRequest,
    next: Next<BoxBody>
) -> Result<ServiceResponse<impl MessageBody>, Error> {

    next.call(req).await
}

pub async fn perform_integration_test<C: HttpServiceFactory + 'static>(
    controller: C,
    req: TestRequest,
    web_data: WebData
) -> Result<IntegrationTestResponse, anyhow::Error>
{
    let req_builder = req.to_request();

    // build the app with the app config and db
    let mut app_builder = web::scope("");

    // add the web data to the app data
    if let Some(config) = web_data.config {
        app_builder = app_builder.app_data(web::Data::new(config));
    }

    if let Some(db) = web_data.db {
        app_builder = app_builder.app_data(web::Data::new(db));
    }

    if let Some(auth) = web_data.auth {
        req_builder.extensions_mut().insert(auth);
    }

    // initialize the app
    let app = test::init_service(
        App::new().service(app_builder.service(controller))
    ).await;
    
    // call the service
    let res = test::call_service(&app, req_builder).await;

    let status = res.status();
    let headers = res.headers().clone();
    let body: Option<bson::Document> = match test::try_read_body_json(res).await {
        Ok(Some(body)) => Some(body),
        Ok(None) => None,
        Err(e) => {
            println!("error: {:?}", e);
            None
        }
    };

    // return the response
    Ok(IntegrationTestResponse {
        status,
        body,
        headers,
    })
}

pub fn get_app_config() -> AppConfig {
    AppConfig{
        port: 3000,
        mongo_url: "mongodb://admin:password@localhost:27017/".to_string(),
        redis_url: "redis://localhost:6379".to_string(),
        enable_swagger: true,
        enable_scheduler: true,
        host: "http://localhost".to_string(),
        jwt_secret: "jwt_secret".to_string(),
    }
}

pub async fn get_db(app_config: &AppConfig) -> Database {
    database::setup_database(
        &app_config.mongo_url, "holo"
    ).await.unwrap()
}

pub fn create_credentials(secret: &str, user_id: bson::oid::ObjectId) -> (String, String) {
    let access_token = sign_access_token(AccessTokenClaims {
        sub: user_id.to_string(),
        exp: 0,
        permissions: vec![],
    }, secret).unwrap();
    let refresh_token = sign_refresh_token(RefreshTokenClaims {
        sub: user_id.to_string(),
        exp: 900000000000,
        version: 0,
    }, secret).unwrap();
    (access_token, refresh_token)
}
