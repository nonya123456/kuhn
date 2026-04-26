use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

// ── types ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
struct SolveRequest {
    card: String,       // "J" | "Q" | "K"
    situation: String,  // "" | "p" | "b" | "pb"
    iterations: u32,
}

#[derive(Serialize, Clone)]
struct SolveResult {
    pass_pct: f64,
    bet_pct: f64,
    ev: f64,
}

enum Job {
    Pending(SolveRequest),
    Done(SolveResult),
}

type AppState = Arc<Mutex<HashMap<String, Job>>>;

// ── cfr+ solver ───────────────────────────────────────────────────────────────

const CARDS: [char; 3] = ['J', 'Q', 'K'];

struct Node {
    regret_sum: [f64; 2],
    strategy_sum: [f64; 2],
}

impl Node {
    fn new() -> Self {
        Self { regret_sum: [0.0; 2], strategy_sum: [0.0; 2] }
    }

    fn strategy(&self) -> [f64; 2] {
        let pos = [self.regret_sum[0].max(0.0), self.regret_sum[1].max(0.0)];
        let sum = pos[0] + pos[1];
        if sum > 0.0 { [pos[0] / sum, pos[1] / sum] } else { [0.5, 0.5] }
    }

    fn avg_strategy(&self) -> [f64; 2] {
        let sum = self.strategy_sum[0] + self.strategy_sum[1];
        if sum > 0.0 {
            [self.strategy_sum[0] / sum, self.strategy_sum[1] / sum]
        } else {
            [0.5, 0.5]
        }
    }
}

fn is_p1_turn(history: &str) -> bool {
    history == "" || history == "pb"
}

fn infoset_key(history: &str, cards: [usize; 2]) -> String {
    let card = CARDS[if is_p1_turn(history) { cards[0] } else { cards[1] }];
    format!("{card}{history}")
}

fn terminal_payoff(history: &str, cards: [usize; 2]) -> Option<f64> {
    let sd = if cards[0] > cards[1] { 1.0 } else { -1.0 };
    match history {
        "pp"  => Some(sd),
        "bp"  => Some(1.0),
        "bb"  => Some(sd * 2.0),
        "pbp" => Some(-1.0),
        "pbb" => Some(sd * 2.0),
        _     => None,
    }
}

fn cfr(nodes: &mut HashMap<String, Node>, cards: [usize; 2], history: &str, p0: f64, p1: f64) -> f64 {
    if let Some(payoff) = terminal_payoff(history, cards) {
        return payoff;
    }

    let key = infoset_key(history, cards);
    let p1_turn = is_p1_turn(history);
    let strat = nodes.entry(key.clone()).or_insert_with(Node::new).strategy();

    let next = |a: &str| format!("{history}{a}");
    let (p_pass, p_bet) = if p1_turn {
        (cfr(nodes, cards, &next("p"), p0 * strat[0], p1),
         cfr(nodes, cards, &next("b"), p0 * strat[1], p1))
    } else {
        (cfr(nodes, cards, &next("p"), p0, p1 * strat[0]),
         cfr(nodes, cards, &next("b"), p0, p1 * strat[1]))
    };

    let node_util = strat[0] * p_pass + strat[1] * p_bet;
    let utils = [p_pass, p_bet];
    let cf_reach = if p1_turn { p1 } else { p0 };
    let my_reach = if p1_turn { p0 } else { p1 };

    let node = nodes.get_mut(&key).unwrap();
    for a in 0..2 {
        let regret = if p1_turn { utils[a] - node_util } else { node_util - utils[a] };
        node.regret_sum[a] = (node.regret_sum[a] + cf_reach * regret).max(0.0);
        node.strategy_sum[a] += my_reach * strat[a];
    }

    node_util
}

fn eval(strat: &HashMap<String, Vec<f64>>, cards: [usize; 2], history: &str) -> f64 {
    if let Some(p) = terminal_payoff(history, cards) {
        return p;
    }
    let key = infoset_key(history, cards);
    let s = strat.get(&key).map_or([0.5; 2], |v| [v[0], v[1]]);
    s[0] * eval(strat, cards, &format!("{history}p")) + s[1] * eval(strat, cards, &format!("{history}b"))
}

fn best_response(strat: &HashMap<String, Vec<f64>>, cards: [usize; 2], history: &str, br: usize) -> f64 {
    if let Some(payoff) = terminal_payoff(history, cards) {
        return if br == 0 { payoff } else { -payoff };
    }

    let p1_turn = is_p1_turn(history);
    let acting = usize::from(!p1_turn);
    let key = infoset_key(history, cards);

    let u_pass = best_response(strat, cards, &format!("{history}p"), br);
    let u_bet  = best_response(strat, cards, &format!("{history}b"), br);

    if acting == br {
        u_pass.max(u_bet)
    } else {
        let s = strat.get(&key).map_or([0.5; 2], |v| [v[0], v[1]]);
        s[0] * u_pass + s[1] * u_bet
    }
}

fn all_deals() -> Vec<[usize; 2]> {
    (0..3).flat_map(|i| (0..3).filter(move |&j| j != i).map(move |j| [i, j])).collect()
}

fn compute_exploitability(strat: &HashMap<String, Vec<f64>>) -> f64 {
    let deals = all_deals();
    let br0 = deals.iter().map(|&c| best_response(strat, c, "", 0)).sum::<f64>() / 6.0;
    let br1 = deals.iter().map(|&c| best_response(strat, c, "", 1)).sum::<f64>() / 6.0;
    (br0 + br1) / 2.0
}

fn run_solver(req: &SolveRequest, on_progress: impl Fn(u32, f64)) -> SolveResult {
    let mut nodes: HashMap<String, Node> = HashMap::new();
    let deals = all_deals();
    let report_every = (req.iterations / 20).max(500);

    for i in 1..=req.iterations {
        for &cards in &deals {
            cfr(&mut nodes, cards, "", 1.0, 1.0);
        }
        if i % report_every == 0 || i == req.iterations {
            let s: HashMap<String, Vec<f64>> = nodes.iter()
                .map(|(k, n)| { let a = n.avg_strategy(); (k.clone(), vec![a[0], a[1]]) })
                .collect();
            on_progress(i, compute_exploitability(&s));
        }
    }

    let strategy: HashMap<String, Vec<f64>> = nodes.iter()
        .map(|(k, n)| { let a = n.avg_strategy(); (k.clone(), vec![a[0], a[1]]) })
        .collect();

    let ev = all_deals().iter().map(|&c| eval(&strategy, c, "")).sum::<f64>() / 6.0;

    let spot_key = format!("{}{}", req.card, req.situation);
    let probs = strategy.get(&spot_key).cloned().unwrap_or(vec![0.5, 0.5]);
    SolveResult { pass_pct: probs[0], bet_pct: probs[1], ev }
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn post_solve(
    State(state): State<AppState>,
    Json(req): Json<SolveRequest>,
) -> Json<serde_json::Value> {
    let job_id = Uuid::new_v4().to_string();
    state.lock().unwrap().insert(job_id.clone(), Job::Pending(req));
    Json(serde_json::json!({ "job_id": job_id }))
}

async fn ws_handler(
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

async fn get_result(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<SolveResult>, StatusCode> {
    match state.lock().unwrap().get(&job_id) {
        Some(Job::Done(r)) => Ok(Json(r.clone())),
        Some(Job::Pending(_)) => Err(StatusCode::ACCEPTED),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let state: AppState = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/solve", post(post_solve))
        .route("/ws/{job_id}", get(ws_handler))
        .route("/result/{job_id}", get(get_result))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await
        .expect("port 3000 already in use — kill the existing process first");
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
