use std::{env, sync::Arc};

use axum::{Router, routing::{post}};
use bb8::Pool;
use bb8_tiberius::ConnectionManager;
use ollama_rs::Ollama;
use tiberius::Config;
// use std::{env, sync::Arc};
// use bb8::Pool;
// use bb8_tiberius::ConnectionManager;
// use tiberius::Config;

use crate::{ocr::deepseek_ocr, state::AppState};


pub async fn get_router() -> Router {
    let conn_str = env::var("ATM_SYNC_DATABASE_CONNECTION_STRING").expect("ATM_SYNC_DATABASE_CONNECTION_STRING missing");
    let config = Config::from_ado_string(&conn_str).unwrap();

    let manager = ConnectionManager::new(config);
    let pool = Pool::builder()
    .max_size(20) // tune this
    .build(manager)
    .await
    .expect("Failed to create DB pool");


    let state = AppState {
        db_pool: Arc::new(pool),
        ollama: Arc::new(Ollama::default()), 
    };

   let sync_routes = Router::new()
        .route("/test", post(deepseek_ocr::mark_complete))
        .route("/deepseek-ocr", post(deepseek_ocr::deepseek_ocr))
        // Apply auth ONLY to these video-sync routes
        // .layer(axum::middleware::from_fn(auth_middleware))
        ;

    return Router::new()
        .nest("/ocr", sync_routes) // Clean nesting
        //Streaming Section
        .layer(axum::extract::DefaultBodyLimit::max(15 * 1024 * 1024 * 1024)) // 15 GB
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state); 
}

