use serde::{Deserialize, Serialize};

use crate::status_code::AppStatusCode;

// 2. Standardized Response Envelope
#[derive(Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: bool,
    pub message: String,
    pub result: Option<T>, // None becomes null in JSON
    #[serde(rename = "statusCode")]
    pub status_code: AppStatusCode,
}

impl<T> ApiResponse<T> {
    // Helper for success responses
    pub fn success(result: T, message: &str) -> Self {
        Self {
            status: true,
            message: message.to_string(),
            result: Some(result),
            status_code: AppStatusCode::Success,
        }
    }

    // Helper for failure responses (result is always null)
    pub fn error(message: String, code: AppStatusCode, result: Option<T>) -> Self {
        Self {
            status: false,
            message,
            // result: None,
            result: result,
            status_code: code,
        }
    }
}

// const COUNTER_NAMES: &[&str] = &["HDFC ", "green", "blue"];
