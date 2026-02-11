use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum AppStatusCode {
    #[serde(rename = "0")]
    Success,
    
    #[serde(rename = "OCR-00001")]
    DbPoolError, // Searchable and descriptive

    #[serde(rename = "OCR-00002")]
    SpExecutionFailed,
    
    #[serde(rename = "OCR-00003")]
    InvalidPayload,
    
    #[serde(rename = "OCR-00004")]
    PathCreation,

    #[serde(rename = "OCR-00005")]
    SpKnownFailed,
    
    // #[serde(rename = "OCR-00005")]
    // InvalidBearerToken,

    // #[serde(rename = "OCR-00006")]
    // MessingBearerToken,

}
