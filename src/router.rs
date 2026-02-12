use std::{collections::HashMap, env, sync::Arc};
use axum::{Router, routing::post};
use bb8::Pool;
use bb8_tiberius::ConnectionManager;
use ollama_rs::Ollama;
use tiberius::Config;

use crate::{ model::TokenResponse, ocr::{copilot, deepseek_ocr, azure_service}, state::AppState};


pub async fn get_router() -> Router {
    // 1. Fetch Environment Variables
    let tenant_id = env::var("TENANT_ID").expect("TENANT_ID missing");
    let client_secret = env::var("CLIENT_SECRET").expect("CLIENT_SECRET missing");
    let client_id = env::var("CLIENT_ID").expect("CLIENT_ID missing");
    let vision_key = env::var("VISION_KEY").expect("VISION_KEY missing");
    let vision_endpoint = env::var("VISION_ENDPOINT").expect("VISION_ENDPOINT missing");
    let document_endpoint = env::var("AZURE_DOCUMENT_INTELLIGENCE_ENDPOINT").expect("AZURE_DOCUMENT_INTELLIGENCE_ENDPOINT missing");
    let document_key = env::var("AZURE_DOCUMENT_INTELLIGENCE_KEY").expect("AZURE_DOCUMENT_INTELLIGENCE_KEY missing");

    let token_data: TokenResponse;
    if tenant_id == ""{    
        // 2. Microsoft OAuth 2.0 Token Request
        let url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant_id);

        let mut params = HashMap::new();
        params.insert("client_id", client_id);
        params.insert("client_secret", client_secret);
        params.insert("grant_type", "client_credentials".to_string());
        params.insert("scope", "https://graph.microsoft.com/.default".to_string());

        let client = reqwest::Client::new();
        
        // We use .expect() here because get_router returns Router, not Result.
        let response = client
            .post(&url)
            .form(&params)
            .send()
            .await
            .expect("CRITICAL: Failed to contact Microsoft Identity platform");
        
        let text = response.text().await.expect("Failed to read body");
        // println!("Response Body: {}", text); 

        // let token_data: TokenResponse = response
        //     .json()
        //     .await
        //     .expect("CRITICAL: Failed to parse Microsoft token response");

        token_data = serde_json::from_str(&text)
        .expect("CRITICAL: Failed to parse token response");
    }else{
        token_data = TokenResponse { access_token: "no id".to_string() };
    }

    // 3. Database Pool Setup
    let conn_str = env::var("ATM_SYNC_DATABASE_CONNECTION_STRING")
        .expect("ATM_SYNC_DATABASE_CONNECTION_STRING missing");
    let config = Config::from_ado_string(&conn_str).expect("Invalid DB connection string");

    let manager = ConnectionManager::new(config);
    let pool = Pool::builder()
        .max_size(20)
        .build(manager)
        .await
        .expect("Failed to create DB pool");

    // 4. State Initialization
    let state = AppState {
        db_pool: Arc::new(pool),
        ollama: Arc::new(Ollama::default()),
        copilot_token: token_data.access_token, // Token shared with all handlers
        azure_vision_endpoint: vision_endpoint,
        azure_vision_key: vision_key,
        azure_document_endpoint: document_endpoint,
        azure_document_key: document_key
    };

    // 5. Route Definition and Nesting
    let sync_routes = Router::new()
        .route("/test", post(deepseek_ocr::mark_complete))
        .route("/deepseek-ocr", post(deepseek_ocr::deepseek_ocr))
        .route("/ask-copilot", post(copilot::ocr_image)) 
        .route("/azure-ocr", post(azure_service::azure_ocr))
        .route("/azure-ocr-document-intelligence", post(azure_service::azure_structured_ocr)); 

    Router::new()
        .nest("/ocr", sync_routes)
        .layer(axum::extract::DefaultBodyLimit::max(15 * 1024 * 1024 * 1024)) // 15 GB
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state) 
}


