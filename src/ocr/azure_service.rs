
use axum::{Json, body::Bytes, extract::{ State}, http::StatusCode, response::IntoResponse};
use reqwest::header::{ HeaderMap, HeaderValue};
use serde_json::{Value, json};

use crate::{ state::AppState};


#[axum::debug_handler]
pub async fn azure_ocr(
    State(state): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    println!("azure_vision_key:{}", state.azure_vision_key);
    println!("azure_vision_endpoint:{}", state.azure_vision_endpoint);

    if body.is_empty() {
        let var_name =  "Empty request body";
        return (StatusCode::BAD_REQUEST, Json(json!({"error": var_name}))).into_response()
    }
    

    // 2. Prepare Azure Vision API Request
    let client = reqwest::Client::new();
    
    // Construct the URL for Image Analysis 4.0 - Read (OCR) feature
    let url = format!(
        // "{}/computervision/imageanalysis:analyze?api-version=2023-02-01-preview&features=read",
        "{}/computervision/imageanalysis:analyze?api-version=2024-02-01&features=read",
        state.azure_vision_endpoint
    );

    let mut headers = HeaderMap::new();
    headers.insert("Ocp-Apim-Subscription-Key", HeaderValue::from_str(&state.azure_vision_key).unwrap());
    headers.insert("Content-Type", HeaderValue::from_static("application/octet-stream"));

    // 3. Send image to Azure
    let response = client
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .await
        .expect("Failed to call Azure Vision API");

    let text = response.text().await.expect("Failed to read body");
    // let result: serde_json::Value = response.json().await.expect("Failed to parse Azure response");
    let result: serde_json::Value = serde_json::from_str(&text)
    .expect("CRITICAL: Failed to parse token response");

    // The response includes text blocks, lines, and words with coordinates
    
    (StatusCode::OK, Json(json!({
        "counter_name": "",
        "value": *Json(result),
    }))).into_response()
}

#[axum::debug_handler]
pub async fn azure_structured_ocr(
    State(state): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    println!("azure_document_key:{}", state.azure_document_key);
    println!("azure_document_endpoint:{}", state.azure_document_endpoint);

    if body.is_empty() {
        let var_name =  "Empty request body";
        return (StatusCode::BAD_REQUEST, Json(json!({"error": var_name}))).into_response()
    }
    let client = reqwest::Client::new();
    
    // Use the Prebuilt Receipt model URL
    let url = format!(
        "{}/documentintelligence/documentModels/prebuilt-receipt:analyze?api-version=2024-11-30",        
        state.azure_document_endpoint.trim_end_matches('/')
    );

    // 1. Send the request
    let response = client.post(&url)
        .header("Ocp-Apim-Subscription-Key", &state.azure_document_key)
        .header("Content-Type", "application/octet-stream")
        .body(body)
        .send()
        .await
        .expect("Failed to reach Azure");

    // 2. Extract the header FIRST (while response still exists)
    let operation_location = response.headers()
        .get("Operation-Location")
        .map(|h| h.to_str().unwrap().to_string()); // Clone it into a String

    // 3. Now check status and consume the body if needed
    if !response.status().is_success() {
        let err_body = response.text().await.unwrap_or_default();
        panic!("Azure Error : {}",  err_body);
    }

    // 4. Use the saved header string
    let operation_url = operation_location
        .expect("Azure returned 202 but missing Operation-Location header");


    loop {
        let status_res = client.get(&operation_url)
            .header("Ocp-Apim-Subscription-Key", &state.azure_document_key)
            .send().await.expect("Polling failed");
            
        let result: serde_json::Value = status_res.json().await.expect("JSON parse failed");
        let status = result["status"].as_str().unwrap_or("failed");

        match status {
            "succeeded" => {
                if let Some(doc) = result["analyzeResult"]["documents"].get(0) {
                    let structured = map_azure_to_atm_json(&doc["fields"]);
                    // return Json(structured).into_response();
                    let result = json!({
                        "structured": structured,
                        "unStructured": &doc["fields"],
                    });
                    return Json(result).into_response();
                }
            },
            "failed" => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Azure analysis failed").into_response();
            },
            _ => { // notStarted or running
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }
    }
}


fn map_azure_to_atm_json(fields: &Value) -> Value {
    let mut dispenser_totals = Vec::new();

    // 1. Map Line Items (Cash Dispenser Totals)
    if let Some(items) = fields["Items"]["valueArray"].as_array() {
        for (i, item) in items.iter().enumerate() {
            let content = item["content"].as_str().unwrap_or("");
            
            // Extract numbers from content like "INC RS.100000 OUT RS.37000"
            // We use a simple helper to find numbers in the string
            let numbers: Vec<f64> = content
                .split(|c: char| !c.is_numeric())
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();

            dispenser_totals.push(json!({
                "num": i + 1,
                "currency": "INR",
                "total": numbers.get(0).unwrap_or(&0.0),      // First number (e.g. 100000)
                "deposited": numbers.get(1).unwrap_or(&0.0),  // Second number
                "left": numbers.get(3).unwrap_or(&0.0),       // Fourth number
                "dispensed": numbers.get(2).unwrap_or(&0.0),  // Third number
            }));
        }
    }

    // 2. Final Structure
    json!({
        "bank_name": fields["MerchantName"]["valueString"].as_str().unwrap_or("Unknown Bank"),
        "transaction_details": {
            "date": fields["TransactionDate"]["valueDate"].as_str().unwrap_or(""),
            "time": fields["TransactionTime"]["valueTime"].as_str().unwrap_or(""),
            "terminal_id": fields["MerchantAddress"]["valueString"].as_str().unwrap_or("")
        },
        "cash_dispenser_totals": dispenser_totals,
        "rejection_status": [] // ATM rejection info usually requires a second pass or regex
    })
}
