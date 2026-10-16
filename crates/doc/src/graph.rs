//! O desenho do grafo de commits: em que coluna (lane) cada commit fica e
//! quais linhas passam por cada linha da lista. Os commits chegam na ordem
//! do `git log --topo-order` (filhos antes dos pais).

/// Um segmento de linha dentro de uma linha da lista: de `from` (lane no
/// topo, ou no meio quando `from_mid`) até `to` (lane embaixo, ou no meio
/// quando `to_mid`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub from: usize,
    pub to: usize,
    pub from_mid: bool,
    pub to_mid: bool,
    /// cor da linha (índice na paleta)
    pub color: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub lane: usize,
    pub color: usize,
    pub segments: Vec<Segment>,
    /// colunas ocupadas nesta linha (para a largura do grafo)
    pub width: usize,
}

/// Limite de colunas (repositórios com centenas de branches abertas não
/// precisam de um grafo de 500 colunas).
pub const MAX_LANES: usize = 32;

pub fn layout<S: AsRef<str>>(commits: &[(S, Vec<S>)]) -> Vec<GraphRow> {
    // lanes[i] = (sha esperado, cor)
    let mut lanes: Vec<Option<(String, usize)>> = Vec::new();
    let mut next_color = 0usize;
    let mut out = Vec::with_capacity(commits.len());
    for (sha, parents) in commits {
        let sha = sha.as_ref();
        let before = lanes.clone();
        let lane = match lanes.iter().position(|l| l.as_ref().is_some_and(|(s, _)| s == sha)) {
            Some(i) => i,
            None => {
                let free = lanes.iter().position(Option::is_none).unwrap_or(lanes.len());
                let color = next_color;
                next_color += 1;
                if free == lanes.len() {
                    lanes.push(Some((sha.to_string(), color)));
                } else if let Some(slot) = lanes.get_mut(free) {
                    *slot = Some((sha.to_string(), color));
                }
                free
            }
        };
        let color = lanes.get(lane).and_then(|l| l.as_ref().map(|(_, c)| *c)).unwrap_or(0);
        let mut segments = Vec::new();
        // linhas que chegam por cima
        for (i, l) in before.iter().enumerate() {
            if let Some((s, c)) = l {
                if s == sha {
                    segments.push(Segment { from: i, to: lane, from_mid: false, to_mid: true, color: *c });
                }
            }
        }
        // outras lanes que esperavam este commit terminam aqui
        for (i, l) in lanes.iter_mut().enumerate() {
            if i != lane && l.as_ref().is_some_and(|(s, _)| s == sha) {
                *l = None;
            }
        }
        // pais
        let mut redirect: Option<(usize, usize, usize)> = None;
        let mut parents = parents.iter().map(AsRef::as_ref);
        match parents.next() {
            Some(first) => {
                // o primeiro pai pode já estar sendo esperado por outra lane
                let waiting = lanes.iter().position(|l| l.as_ref().is_some_and(|(s, _)| s == first));
                if let Some(j) = waiting.filter(|j| *j > lane) {
                    // a lane da direita desemboca nesta (o grafo puxa para a esquerda)
                    let c = lanes.get(j).and_then(|l| l.as_ref().map(|x| x.1)).unwrap_or(color);
                    if let Some(slot) = lanes.get_mut(j) {
                        *slot = None;
                    }
                    if let Some(slot) = lanes.get_mut(lane) {
                        *slot = Some((first.to_string(), color));
                    }
                    segments.push(Segment { from: lane, to: lane, from_mid: true, to_mid: false, color });
                    redirect = Some((j, lane, c));
                } else if let Some(j) = waiting {
                    segments.push(Segment {
                        from: lane,
                        to: j,
                        from_mid: true,
                        to_mid: false,
                        color: lanes.get(j).and_then(|l| l.as_ref().map(|x| x.1)).unwrap_or(color),
                    });
                    if let Some(slot) = lanes.get_mut(lane) {
                        *slot = None;
                    }
                } else {
                    if let Some(slot) = lanes.get_mut(lane) {
                        *slot = Some((first.to_string(), color));
                    }
                    segments.push(Segment { from: lane, to: lane, from_mid: true, to_mid: false, color });
                }
            }
            None => {
                if let Some(slot) = lanes.get_mut(lane) {
                    *slot = None;
                }
            }
        }
        for p in parents {
            let j = match lanes.iter().position(|l| l.as_ref().is_some_and(|(s, _)| s == p)) {
                Some(j) => j,
                None => {
                    let free = lanes.iter().position(Option::is_none).unwrap_or(lanes.len());
                    if free >= MAX_LANES {
                        continue;
                    }
                    let c = next_color;
                    next_color += 1;
                    if free == lanes.len() {
                        lanes.push(Some((p.to_string(), c)));
                    } else if let Some(slot) = lanes.get_mut(free) {
                        *slot = Some((p.to_string(), c));
                    }
                    free
                }
            };
            let c = lanes.get(j).and_then(|l| l.as_ref().map(|x| x.1)).unwrap_or(color);
            segments.push(Segment { from: lane, to: j, from_mid: true, to_mid: false, color: c });
        }
        if let Some((from, to, c)) = redirect {
            segments.push(Segment { from, to, from_mid: false, to_mid: false, color: c });
        }
        // linhas que só passam
        for (i, l) in before.iter().enumerate() {
            if let Some((s, c)) = l {
                if s != sha && lanes.get(i).is_some_and(|x| x.as_ref().is_some_and(|(t, _)| t == s)) {
                    segments.push(Segment { from: i, to: i, from_mid: false, to_mid: false, color: *c });
                }
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        let width = segments.iter().map(|s| s.from.max(s.to) + 1).max().unwrap_or(1).max(lane + 1);
        out.push(GraphRow { lane, color, segments, width });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(sha: &'static str, parents: &[&'static str]) -> (&'static str, Vec<&'static str>) {
        (sha, parents.to_vec())
    }

    #[test]
    fn linear_history_stays_in_one_lane() {
        let rows = layout(&[c("c", &["b"]), c("b", &["a"]), c("a", &[])]);
        assert!(rows.iter().all(|r| r.lane == 0 && r.width == 1));
        assert!(rows.last().is_some_and(|r| r.segments.iter().all(|s| !s.from_mid)), "raiz sem linha para baixo");
    }

    #[test]
    fn a_merge_opens_and_closes_a_second_lane() {
        // m = merge(b, f); f e b saem de a
        let rows = layout(&[c("m", &["b", "f"]), c("f", &["a"]), c("b", &["a"]), c("a", &[])]);
        assert_eq!(rows[0].lane, 0);
        assert_eq!(rows[1].lane, 1, "o lado do merge abre uma lane");
        assert_eq!(rows[2].lane, 0);
        assert_eq!(rows[3].lane, 0, "as duas lanes convergem em a");
        assert!(rows[0].segments.iter().any(|s| s.from == 0 && s.to == 1 && s.from_mid));
        assert!(rows[2].segments.iter().any(|s| s.from == 1 && s.to == 0 && !s.from_mid), "a lane 1 desemboca na 0");
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_lanes_stay_bounded(edges in proptest::collection::vec((0usize..40, proptest::collection::vec(0usize..40, 0..3)), 0..80)) {
            let commits: Vec<(String, Vec<String>)> = edges.iter().map(|(s, ps)| (s.to_string(), ps.iter().map(|p| p.to_string()).collect())).collect();
            for r in layout(&commits) {
                proptest::prop_assert!(r.width <= MAX_LANES + 80);
            }
        }
    }
}
