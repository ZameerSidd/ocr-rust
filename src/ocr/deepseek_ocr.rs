use std::collections::HashMap;
use axum::{Json, body::Bytes, extract::{Query, State}, http::StatusCode, response::{IntoResponse, Response}};
use base64::{Engine, engine::general_purpose};
use futures_util::TryStreamExt;
use ollama_rs::{ generation::{completion::request::GenerationRequest, images::Image}};
use serde_json::{Map, Value, json};
use tiberius::{Query as SqlQuery,  QueryItem };
use tokio::{fs, io::AsyncWriteExt};

use crate::{constant::ApiResponse, model::SqlParam, state::AppState, status_code::AppStatusCode};

pub async fn mark_complete(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    // 1. Validation
    let daily_run_atm_id = match params.get("dailyRunAtmId") {
        Some(v) => v.to_string(),
        None => {
            let res = ApiResponse::<Value>::error("Missing Daily Run Atm Id".to_string(), AppStatusCode::InvalidPayload, None);
            return (StatusCode::BAD_REQUEST, Json(res)).into_response();
        }
    };

    let atm_id: i64 = daily_run_atm_id
    .parse()                // Parse to i64
    .unwrap_or(0); 

    if atm_id == 0 {
        let res = ApiResponse::<Value>::error("invalid daily run atm id".to_string(), AppStatusCode::InvalidPayload, None);
        return (StatusCode::BAD_REQUEST, Json(res)).into_response();
    }

    let chunk_dir = format!("{}/{}", "", daily_run_atm_id);
    let output_dir = format!("{}", "");
    let output_path = format!("{}/{}.mp4", output_dir, daily_run_atm_id);

    // 2. File Processing (Merging Chunks)
    if let Err(e) = fs::create_dir_all(&output_dir).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Value>::error(format!("Dir Error: {}", e), AppStatusCode::PathCreation, None))).into_response();
    }

    let mut output_file = match tokio::fs::File::create(&output_path).await {
        Ok(f) => f,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Value>::error(e.to_string(), AppStatusCode::PathCreation, None))).into_response(),
    };

    let mut entries = match tokio::fs::read_dir(&chunk_dir).await {
        Ok(e) => e,
        Err(e) => return (StatusCode::NOT_FOUND, Json(ApiResponse::<Value>::error(format!("Chunks not found. {}", e.to_string()), AppStatusCode::PathCreation, None))).into_response(),
    };

    let mut chunks = vec![];
    while let Ok(Some(entry)) = entries.next_entry().await {
        chunks.push(entry.path());
    }
    chunks.sort(); // Crucial for video order

    for chunk in chunks {
        if let Ok(data) = tokio::fs::read(&chunk).await {
            let _ = output_file.write_all(&data).await;
        }
    }

    let url = "";

    // 4. DB Update
    let mut client = match state.db_pool.get().await {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Value>::error("DB Pool Error".to_string(), AppStatusCode::DbPoolError, None))).into_response(),
    };

    
    let params = vec![
        ("dailyRunAtmId", SqlParam::I64(atm_id)),
        ("fileName", SqlParam::String(format!("{}.mp4", daily_run_atm_id))),
        ("filePath", SqlParam::String(output_path)),
        ("url", SqlParam::String(url.to_string())),
    ];

    let response = execute_sp_dynamic(&mut client, "usp_Complete_Chunk_Video", &params).await;

    // 5. Finalize & Cleanup
    match response {
        Ok((true, msg, data)) => {
            // Delete temporary chunks
            // let _ = tokio::fs::remove_dir_all(chunk_dir).await;

            let result_data = data.unwrap_or_else(|| json!({})); 
            (StatusCode::OK, Json(ApiResponse::success(result_data, &msg))).into_response()
        }
        Ok((false, msg, data)) => {
            let result_data = data.unwrap_or_else(|| json!({})); 
            (StatusCode::BAD_REQUEST, Json(ApiResponse::error(msg, AppStatusCode::SpExecutionFailed, Some(result_data)))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::<Value>::error(e.to_string(), AppStatusCode::SpKnownFailed, None))).into_response()
        }
    }
}

async fn execute_sp_dynamic(
    client: &mut bb8::PooledConnection<'_, bb8_tiberius::ConnectionManager>,
    sp_name: &str,
    params: &[(&str, SqlParam)], 
) -> Result<(bool, String, Option<Value>), String> {

    let param_bindings = params.iter()
    .map(|(name, value)| {
        let formatted_value = match value {
            SqlParam::String(v) => format!("'{}'", v), // Wrap strings in single quotes
            // SqlParam::I32(v) => v.to_string(),
            SqlParam::I64(v) => v.to_string(),
            // SqlParam::F32(v) => v.to_string(),
            // SqlParam::F64(v) => v.to_string(),
            // SqlParam::Bool(v) => if *v { "1".to_string() } else { "0".to_string() },
            // SqlParam::Null => "NULL".to_string(),
            // SqlParam::DateTime(v) => format!("'{}'", v.format("%Y-%m-%d %H:%M:%S")),
            // SqlParam::DateTime(v) => format!("'{}'", v.format("%Y-%m-%d %H:%M:%S.%f")),
        };
        format!("@{} = {}", name, formatted_value)
    })
    .collect::<Vec<_>>()
    .join(", ");



    // println!("param binding:{}" ,param_bindings);
    // 2. Add semicolons and ensure 'EXEC' isn't jammed against the DECLARE
    let sql = format!(
        "DECLARE @status BIT, @msg VARCHAR(1000); \
        EXEC {} {}, @retStatus = @status OUTPUT, @retMessage = @msg OUTPUT; \
        SELECT @status AS retStatus, @msg AS retMessage;",
        sp_name, param_bindings
    );


    let mut query = SqlQuery::new(sql);
    
    // 3. Bind the Rust values to the @P placeholder markers
    for (_, value) in params.iter() {
        value.bind_to_query(&mut query);
    }
    
    // let mut stream = query.query(client).await.map_err(|e| e.to_string())?;
    let mut stream = match query.query(client).await {
        Ok(s) => s,
        Err(e) => {
            return Ok((false, e.to_string(), None));
        }
    };
    
    // let mut data_json = None;
    // let mut status = false;
    // let mut msg = "Unknown".to_string();

    let mut data_json: Option<Value> = None;
    let mut status = false;
    let mut msg = String::new();

    loop {
    match stream.try_next().await {
        Ok(Some(item)) => {
            match item {
                QueryItem::Row(row) => {
                    // Check if the FIRST column is named "retStatus"
                    let first_col_name = row.columns().get(0).map(|c| c.name()).unwrap_or("");

                    if first_col_name == "retStatus" {
                        // This is your standard footer (Result Index 0 OR 1)
                        status = row.get::<bool, _>(0).unwrap_or(false);
                        msg = row.get::<&str, _>(1).unwrap_or("Unknown").to_string();
                    } else {
                        // This is dynamic data from inside the SP
                        let mut map = Map::new();
                        for (i, column) in row.columns().iter().enumerate() {
                            let name = column.name().to_string();
                            let val = match row.try_get::<&str, _>(i) {
                                Ok(Some(s)) => json!(s),
                                _ => match row.try_get::<i32, _>(i) {
                                    Ok(Some(n)) => json!(n),
                                    _ => match row.try_get::<i64, _>(i) { // Added i64 for BigInt support
                                        Ok(Some(n)) => json!(n),
                                        _ => Value::Null,
                                    },
                                },
                            };
                            map.insert(name, val);
                        }
                        data_json = Some(Value::Object(map));
                    }
                }
                QueryItem::Metadata(_) => continue,
            }
        }
        Ok(None) => break, 
        Err(e) => return Ok((false, e.to_string(), None)),
        }
    }

    // Return the values collected during the loop
    Ok((status, msg, data_json))

    // Ok((status, msg, if status { data_json } else { None }))
}


pub async fn deepseek_ocr(
    State(state): State<AppState>, 
    Query(params): Query<HashMap<String, String>>,
    body: Bytes,
) -> impl IntoResponse {
    // 1. Validation Logic
    let counter_name = match params.get("counter_name") {
        Some(v) if v == "ATM" => v,
        _ => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid or missing counter_name"}))).into_response(),
    };
    let model = match params.get("model_name") {
        Some(v) if !v.trim().is_empty() => v,
        _ => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid or missing model_name"}))).into_response(),
    };

    // 2. Setup Ollama (Assuming default localhost:11434)
    let ollama = state.ollama;
    // let model = "deepseek-ocr"; // Ensure this matches your downloaded model name
    //let model = "qwen2.5vl:3b-q4_K_M"; // Ensure this matches your downloaded model name

    // 3. Prepare Image (Base64 encoding is required for Ollama's vision API)
    let b64_image = general_purpose::STANDARD.encode(&body);

    // Wrap the Base64 string in the Image struct expected by the SDK
    let image_obj = Image::from_base64(&b64_image);

    // 4. Construct Prompt
    // let prompt = "Extract all text from this image and format it as Markdown.";
    let prompt = "extract image text and convert into json.";
    let request = GenerationRequest::new(model.to_string(), prompt.to_string())
    .add_image(image_obj); // Now passing the correct Type

    // 5. Execute OCR
    match ollama.generate(request).await {
        Ok(res) => {
            let ocr_text = res.response;

            // 1. Clean the string to extract only the JSON part
            let json_text = ocr_text
                .split("```json")
                .nth(1)
                .and_then(|c| c.split("```").next())
                .ok_or("No JSON found").unwrap()
                .replace('\n', "") // Remove all newlines
                .replace('\r', "")
                .trim()
                .to_string();
            
            let json_object: Value = serde_json::from_str(&json_text).unwrap_or(json!({"error": "parse_failed"}));

            if let Some(bank) = json_object.get("bank") {
                println!("Extracted Bank: {}", bank);
            }

            (StatusCode::OK, Json(json!({
                "counter_name": counter_name,
                "ocr_data_json": json_object, 
                "ocr_data": ocr_text, 
                "ocr_data_json_text": json_text, 
                "status": "success"
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": format!("Ollama error: {}", e)
            }))).into_response()
        }
    }


    // match ollama.generate(request).await {
    // Ok(res) => {
    //     // 1. Clean the string (remove markdown block if the model adds it)
    //     let cleaned_text = res.response
    //         .replace("```json", "")
    //         .replace("```", "")
    //         .trim()
    //         .to_string();

    //     // 2. Parse the string into a Value to ensure it's valid JSON
    //     let ocr_json: Value = serde_json::from_str(&cleaned_text)
    //         .unwrap_or_else(|_| json!({ "error": "Failed to parse OCR result", "raw": cleaned_text }));

    //     // 3. Return as a proper nested JSON object
    //     (StatusCode::OK, Json(json!({
    //         "counter_name": counter_name,
    //         "ocr_data": ocr_json, 
    //         "status": "success"
    //     }))).into_response()
    // }
    // Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    // }

}
