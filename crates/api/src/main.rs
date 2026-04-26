use std::{collections::HashMap, sync::{Arc, Mutex}};

use axum::{Router, routing::{get, post}};
use tower_http::cors::CorsLayer;

mod handlers;
mod solver;

#[tokio::main]
async fn main() {
    let state: handlers::AppState = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/solve", post(handlers::post_solve))
        .route("/ws/{job_id}", get(handlers::ws_handler))
        .route("/result/{job_id}", get(handlers::get_result))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await
        .expect("port 3000 already in use — kill the existing process first");
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
