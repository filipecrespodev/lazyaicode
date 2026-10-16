//! Timings of the hot read paths against a big generated repo (5k commits,
//! a 10k-line file edited every 5th commit, a 300-commit side branch).
//! Ignored by default; run with
//! `cargo test --release --lib perf_report -- --ignored --nocapture`
//! (`GITHYDRA_PERF_REPO=<path>` reuses a repo instead of generating one).

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use super::*;

/// Streams the history into `git fast-import`: seconds instead of minutes.
fn generate(dir: &std::path::Path) {
    let git = |args: &[&str]| {
        let ok = Command::new("git").current_dir(dir).args(args).status().unwrap().success();
        assert!(ok, "git {args:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    let mut child = Command::new("git").current_dir(dir).args(["fast-import", "--quiet"]).stdin(Stdio::piped()).spawn().unwrap();
    let mut out = std::io::BufWriter::new(child.stdin.take().unwrap());
    let data = |out: &mut dyn Write, b: &[u8]| {
        writeln!(out, "data {}", b.len()).unwrap();
        out.write_all(b).unwrap();
        out.write_all(b"\n").unwrap();
    };
    let mut big: Vec<String> = (0..10_000).map(|i| format!("line {i} original content here")).collect();
    let mut seed: u64 = 1;
    let mut rand = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize % n
    };
    let t = 1_600_000_000;
    for c in 1..=5300u32 {
        let (branch, from) = if c <= 5000 { ("main", c - 1) } else { ("side", if c == 5001 { 4000 } else { c - 1 }) };
        write!(out, "commit refs/heads/{branch}\nmark :{c}\n").unwrap();
        write!(out, "author A <a@x> {0} +0000\ncommitter A <a@x> {0} +0000\n", t + c * 60).unwrap();
        data(&mut out, format!("commit {c}").as_bytes());
        if from > 0 {
            writeln!(out, "from :{from}").unwrap();
        }
        if c <= 5000 && c % 5 == 1 {
            for _ in 0..5 {
                let i = rand(big.len());
                big[i] = format!("line {i} edited in {c}");
            }
            out.write_all(b"M 100644 inline big.txt\n").unwrap();
            data(&mut out, (big.join("\n") + "\n").as_bytes());
        }
        writeln!(out, "M 100644 inline src/f{}.txt", c % 200).unwrap();
        data(&mut out, format!("v{c}\n").as_bytes());
    }
    drop(out);
    assert!(child.wait().unwrap().success());
    git(&["-c", "core.autocrlf=false", "checkout", "-q", "main"]);
}

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    eprintln!("{label:40} {:>8.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    r
}

#[test]
#[ignore]
fn perf_report() {
    let tmp = tempfile::tempdir().unwrap();
    let path = match std::env::var("GITHYDRA_PERF_REPO") {
        Ok(path) => path,
        Err(_) => {
            time("generate the repo", || generate(tmp.path()));
            tmp.path().to_string_lossy().into_owned()
        }
    };
    let repo = git2::Repository::open(&path).unwrap();
    let q = |skip| log::LogQuery { skip, limit: 200, branch_filter: None };
    time("log page 0", || log::commit_log(&repo, &q(0)).unwrap());
    time("log page 10 (skip 2000)", || log::commit_log(&repo, &q(2000)).unwrap());
    time("log 25 pages refetch (no cache: before)", || {
        for i in 0..25 {
            log::commit_log(&repo, &q(i * 200)).unwrap();
        }
    });
    let mut cache = log::LogCache::default();
    time("log page 0 (cold cache)", || log::commit_log_cached(&repo, &q(0), &mut cache).unwrap());
    time("log 25 pages refetch (cached)", || {
        for i in 0..25 {
            log::commit_log_cached(&repo, &q(i * 200), &mut cache).unwrap();
        }
    });
    let fh = |p: &str| history::FileHistoryQuery { path: p.into(), start_sha: None, cursor: None, limit: 100 };
    time("file history big.txt (100)", || history::file_history(&repo, &fh("big.txt")).unwrap());
    time("file history src/f7.txt (100)", || history::file_history(&repo, &fh("src/f7.txt")).unwrap());
    time("file history missing (scan cap)", || history::file_history(&repo, &fh("nope")).unwrap());
    time("libgit2 blame of a 1000-line window (before)", || {
        let mut opts = git2::BlameOptions::new();
        opts.min_line(1).max_line(1000);
        repo.blame_file(std::path::Path::new("big.txt"), Some(&mut opts)).unwrap();
    });
    let bc = history::BlameCache::default();
    time("blame big.txt lines 1..1000 (cold)", || history::blame(&repo, "big.txt", None, 1, 1000, &bc).unwrap());
    time("blame big.txt lines 9001..10000 (cached)", || history::blame(&repo, "big.txt", None, 9001, 1000, &bc).unwrap());
    time("blame big.txt all 10 chunks (cached)", || {
        for i in 0..10 {
            history::blame(&repo, "big.txt", None, 1 + i * 1000, 1000, &bc).unwrap();
        }
    });
    time("compare main..side", || history::compare_refs(&repo, "main", "side", 200).unwrap());
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    time("commit diff HEAD", || diff::commit_diff(&repo, &head.id().to_string()).unwrap());
    let bigc = repo.revparse_single("main~4").unwrap().id().to_string();
    time("commit diff touching big.txt", || diff::commit_diff(&repo, &bigc).unwrap());
    time("commit file diff big.txt", || history::commit_file_diff(&repo, &bigc, "big.txt", None).unwrap());
    time("upstream/status", || repo::status(&repo).unwrap());
    time("working status", || working::working_status(&repo).unwrap());
    time("list branches", || branch::list_branches(&repo).unwrap());
    // Edit the big file in the workdir.
    let wd = repo.workdir().unwrap().join("big.txt");
    let orig = std::fs::read_to_string(&wd).unwrap();
    let mut lines: Vec<String> = orig.lines().map(String::from).collect();
    for i in (0..lines.len()).step_by(500) {
        lines[i] = format!("{} changed", lines[i]);
    }
    std::fs::write(&wd, lines.join("\n") + "\n").unwrap();
    time("workdir diff (unstaged, big edit)", || diff::workdir_diff(&repo, false).unwrap());
    time("apply_lines stage one line", || {
        partial::apply_lines(
            &repo,
            "big.txt",
            partial::LineAction::Stage,
            &partial::LineSelection { old_lines: vec![1], new_lines: vec![1], ..Default::default() },
        )
        .unwrap()
    });
    let mut r2 = git2::Repository::open(&path).unwrap();
    time("undo capture", || undo::capture(&mut r2).unwrap());
    time("repo stats", || stats::repo_stats(&repo, 1_700_000_000).unwrap());
    std::fs::write(&wd, orig).unwrap();
    let mut idx = repo.index().unwrap();
    idx.read(true).unwrap();
    repo.reset(head.as_object(), git2::ResetType::Mixed, None).unwrap();
}
