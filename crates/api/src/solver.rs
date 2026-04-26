use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct SolveRequest {
    pub card: String,
    pub situation: String,
    pub iterations: u32,
}

#[derive(Serialize, Clone)]
pub struct SolveResult {
    pub pass_pct: f64,
    pub bet_pct: f64,
    pub ev: f64,
}

pub const CARDS: [char; 3] = ['J', 'Q', 'K'];

pub struct Node {
    regret_sum: [f64; 2],
    strategy_sum: [f64; 2],
}

impl Node {
    pub fn new() -> Self {
        Self { regret_sum: [0.0; 2], strategy_sum: [0.0; 2] }
    }

    pub fn strategy(&self) -> [f64; 2] {
        let pos = [self.regret_sum[0].max(0.0), self.regret_sum[1].max(0.0)];
        let sum = pos[0] + pos[1];
        if sum > 0.0 { [pos[0] / sum, pos[1] / sum] } else { [0.5, 0.5] }
    }

    pub fn avg_strategy(&self) -> [f64; 2] {
        let sum = self.strategy_sum[0] + self.strategy_sum[1];
        if sum > 0.0 {
            [self.strategy_sum[0] / sum, self.strategy_sum[1] / sum]
        } else {
            [0.5, 0.5]
        }
    }
}

pub fn is_p1_turn(history: &str) -> bool {
    history == "" || history == "pb"
}

pub fn infoset_key(history: &str, cards: [usize; 2]) -> String {
    let card = CARDS[if is_p1_turn(history) { cards[0] } else { cards[1] }];
    format!("{card}{history}")
}

pub fn terminal_payoff(history: &str, cards: [usize; 2]) -> Option<f64> {
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

pub fn cfr(nodes: &mut HashMap<String, Node>, cards: [usize; 2], history: &str, p0: f64, p1: f64) -> f64 {
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

pub fn eval(strat: &HashMap<String, Vec<f64>>, cards: [usize; 2], history: &str) -> f64 {
    if let Some(p) = terminal_payoff(history, cards) {
        return p;
    }
    let key = infoset_key(history, cards);
    let s = strat.get(&key).map_or([0.5; 2], |v| [v[0], v[1]]);
    s[0] * eval(strat, cards, &format!("{history}p")) + s[1] * eval(strat, cards, &format!("{history}b"))
}

pub fn best_response(strat: &HashMap<String, Vec<f64>>, cards: [usize; 2], history: &str, br: usize) -> f64 {
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

pub fn all_deals() -> Vec<[usize; 2]> {
    (0..3).flat_map(|i| (0..3).filter(move |&j| j != i).map(move |j| [i, j])).collect()
}

pub fn compute_exploitability(strat: &HashMap<String, Vec<f64>>) -> f64 {
    let deals = all_deals();
    let br0 = deals.iter().map(|&c| best_response(strat, c, "", 0)).sum::<f64>() / 6.0;
    let br1 = deals.iter().map(|&c| best_response(strat, c, "", 1)).sum::<f64>() / 6.0;
    (br0 + br1) / 2.0
}

pub fn run_solver(req: &SolveRequest, on_progress: impl Fn(u32, f64)) -> SolveResult {
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
