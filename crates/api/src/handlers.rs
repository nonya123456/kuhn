use std::{collections::HashMap, sync::{Arc, Mutex}};

use axum::{
    Json, extract::{Path, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::Response,
};
use uuid::Uuid;

use crate::solver::{run_solver, SolveRequest, SolveResult};

pub enum Job {
    Pending(SolveRequest),
    Done(SolveResult),
}

pub type AppState = Arc<Mutex<HashMap<String, Job>>>;

pub async fn post_solve(
    State(state): State<AppState>,
    Json(req): Json<SolveRequest>,
) -> Json<serde_json::Value> {
    let job_id = Uuid::new_v4().to_string();
    state.lock().unwrap().insert(job_id.clone(), Job::Pending(req));
    Json(serde_json::json!({ "job_id": job_id }))
}

pub async fn ws_handler(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state, job_id))
}

async fn handle_ws(mut socket: WebSocket, state: AppState, job_id: String) {
    let req = match state.lock().unwrap().get(&job_id) {
        Some(Job::Pending(r)) => r.clone(),
        _ => return,
    };
    let total = req.iterations;

    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<(u32, f64)>();

    let result = tokio::task::spawn_blocking(move || {
        run_solver(&req, |iter, expl| { let _ = progress_tx.send((iter, expl)); })
    });

    for (iter, expl) in progress_rx {
        let msg = serde_json::json!({
            "type": "progress",
            "iteration": iter,
            "total": total,
            "exploitability": expl,
        });
        if socket.send(Message::Text(msg.to_string().into())).await.is_err() {
            return;
        }
    }

    let solve_result = result.await.unwrap();
    state.lock().unwrap().insert(job_id, Job::Done(solve_result));

    let _ = socket.send(Message::Text(
        serde_json::json!({ "type": "done" }).to_string().into()
    )).await;
}

pub async fn get_result(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<SolveResult>, StatusCode> {
    match state.lock().unwrap().get(&job_id) {
        Some(Job::Done(r)) => Ok(Json(r.clone())),
        Some(Job::Pending(_)) => Err(StatusCode::ACCEPTED),
        None => Err(StatusCode::NOT_FOUND),
    }
}
