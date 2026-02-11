use std::sync::Arc;

use bb8::Pool;
use bb8_tiberius::ConnectionManager;
use ollama_rs::Ollama;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Arc<Pool<ConnectionManager>>,
    pub ollama: Arc<Ollama>, 
}