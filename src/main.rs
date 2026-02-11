use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;
use dotenv::dotenv;
mod router;
mod ocr;
mod constant;
mod status_code;
mod state;
mod model;

#[tokio::main]
async  fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

    let rt = router::get_router().await;

    let address = SocketAddr::from(([0, 0, 0, 0], 8791));
    println!("listening on {}", address);

    let listener = tokio::net::TcpListener::bind(address).await.unwrap();

    axum::serve(listener, rt).await.unwrap();
}
