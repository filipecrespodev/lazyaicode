//! Fuzzing de bolso, no Rust estável: as sementes de `seeds/<alvo>/` e
//! entradas pseudoaleatórias (mutações das sementes e bytes soltos). Pega
//! regressões no CI sem nightly; o fuzzing de verdade é `cargo fuzz run`.

type Target = fn(&[u8]);

const TARGETS: &[(&str, Target)] = &[
    ("transcript", lazyaicode_fuzz::transcript),
    ("frame", lazyaicode_fuzz::frame),
    ("patch", lazyaicode_fuzz::patch),
    ("graph", lazyaicode_fuzz::graph),
    ("term", lazyaicode_fuzz::term),
    ("acp_translate", lazyaicode_fuzz::acp_translate),
    ("session", lazyaicode_fuzz::session),
    ("git_frame", lazyaicode_fuzz::git_frame),
    ("frontmatter", lazyaicode_fuzz::frontmatter),
    ("params", lazyaicode_fuzz::params),
    ("wslpath", lazyaicode_fuzz::wslpath),
    ("slash_rank", lazyaicode_fuzz::slash_rank),
];

/// xorshift: determinístico, sem dependência.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn seeds(name: &str) -> Vec<Vec<u8>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("seeds").join(name);
    let mut out: Vec<Vec<u8>> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| std::fs::read(e.path()).ok()).collect();
    out.push(Vec::new());
    out
}

fn mutate(rng: &mut Rng, mut v: Vec<u8>) -> Vec<u8> {
    let n = (rng.next() % 8) as usize;
    for _ in 0..n {
        match rng.next() % 4 {
            0 if !v.is_empty() => {
                let i = (rng.next() as usize) % v.len();
                v[i] = rng.next() as u8;
            }
            1 => {
                let i = if v.is_empty() { 0 } else { (rng.next() as usize) % v.len() };
                const BYTES: &[u8] = b"{}[]\":,\\\n\x1b0";
                v.insert(i, BYTES[(rng.next() as usize) % BYTES.len()]);
            }
            2 if !v.is_empty() => {
                let i = (rng.next() as usize) % v.len();
                v.truncate(i);
            }
            _ => {
                let len = v.len();
                let extra: Vec<u8> = v.iter().take(len.min(64)).copied().collect();
                v.extend(extra);
            }
        }
    }
    v
}

#[test]
fn every_target_survives_seeds_and_mutations() {
    let iterations: usize = std::env::var("FUZZ_SMOKE_ITERS").ok().and_then(|s| s.parse().ok()).unwrap_or(2000);
    for (name, target) in TARGETS {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ name.len() as u64);
        let base = seeds(name);
        for s in &base {
            target(s);
        }
        for i in 0..iterations {
            let input = if i % 3 == 0 {
                (0..(rng.next() % 256)).map(|_| rng.next() as u8).collect()
            } else {
                let s = &base[(rng.next() as usize) % base.len()];
                mutate(&mut rng, s.clone())
            };
            target(&input);
        }
    }
}
