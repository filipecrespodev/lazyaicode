//! Cost of going through the agent pipe vs calling the actor in-process, on
//! a real repo:
//! `LAZYAICODE_BENCH_REPO=/path cargo test --release --test agent_bench -- --ignored --nocapture`
#![cfg(all(feature = "gui", unix))]

use std::time::Instant;

use lazyaicode_lib::git_client::git::log::LogQuery;
use lazyaicode_lib::git_client::state::AppState;
use lazyaicode_lib::git_client::wslpath::to_unc;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[tokio::test]
#[ignore]
async fn pipe_overhead() {
    std::env::set_var("LAZYAICODE_AGENT_BIN", env!("CARGO_BIN_EXE_lazyaicode-agent"));
    let repo = std::env::var("LAZYAICODE_BENCH_REPO").unwrap_or_else(|_| ".".into());
    let repo = std::fs::canonicalize(repo).unwrap().to_string_lossy().into_owned();
    let state = AppState::default();

    let local = state.open_local("local".into(), repo.clone()).unwrap();
    let remote = state.open_remote("Bench", repo.clone()).await.unwrap();
    assert_eq!(to_unc("Bench", &local.path), remote.path);

    for (name, id) in [("in-process", &local.repo_id), ("via agent ", &remote.repo_id)] {
        let actor = state.actor(id).unwrap();
        let (mut status, mut log) = (vec![], vec![]);
        for _ in 0..30 {
            let t = Instant::now();
            actor.status().await.unwrap();
            status.push(t.elapsed().as_secs_f64() * 1e3);
            let t = Instant::now();
            actor.log(LogQuery { skip: 0, limit: 500, branch_filter: None }).await.unwrap();
            log.push(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{name}: status {:.2} ms, log(500) {:.2} ms (median of 30)", median(status), median(log));
    }
}
