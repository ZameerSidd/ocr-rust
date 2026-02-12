use axum::{Json, extract::{Multipart, State},  response::IntoResponse};
use reqwest::header::{AUTHORIZATION};

use crate::{ state::AppState};

#[axum::debug_handler]
pub async fn ocr_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // 1. Extract image bytes from multipart
    let mut image_bytes = Vec::new();
    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.name() == Some("image") {
            image_bytes = field.bytes().await.unwrap().to_vec();
            break;
        }
    }

    // 2. Upload image to OneDrive (Required for Copilot to "see" the file)
    // Endpoint: https://graph.microsoft.com
    let upload_url = "https://graph.microsoft.com";
    let client = reqwest::Client::new();
    
    let upload_res = client.put(upload_url)
        .header(AUTHORIZATION, format!("Bearer {}", state.copilot_token))
        .body(image_bytes)
        .send()
        .await
        .expect("Upload failed");

    let drive_item: serde_json::Value = upload_res.json().await.unwrap();
    let web_url = drive_item["webUrl"].as_str().unwrap();

    // 3. Call Copilot Chat with the prompt
    let chat_url = "https://graph.microsoft.com"; 
    let ocr_prompt = format!(
        "Using the image at {}, extract text from image and convert into json format.", 
        web_url
    );

    let response = client.post(chat_url)
        .header(AUTHORIZATION, format!("Bearer {}", state.copilot_token))
        .json(&serde_json::json!({
            "message": { "text": ocr_prompt }
        }))
        .send()
        .await
        .expect("Copilot call failed");

    Json(response.json::<serde_json::Value>().await.unwrap())
}

