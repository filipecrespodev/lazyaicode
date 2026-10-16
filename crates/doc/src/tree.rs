//! A árvore da barra lateral: agrupa as threads (por projeto ou por data),
//! decide o que começa aberto e achata o que está visível numa lista de
//! linhas — a lista é o que a navegação por teclado percorre.

use crate::model::{Project, Status, ThreadView};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeMode {
    Project,
    Recent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Activity {
    Awaiting,
    Running,
    Error,
    Unread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Favorites,
    Project,
    Time,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub key: String,
    /// Nome do projeto, ou a chave de tradução (`tree.today`, `tree.favorites`).
    pub label: String,
    pub kind: GroupKind,
    pub project: Option<Project>,
    pub threads: Vec<ThreadView>,
    /// o que chama atenção dentro do grupo, para aparecer mesmo recolhido
    pub activity: Option<Activity>,
}

#[derive(Debug, Clone)]
pub enum Row {
    Group { key: String, group: usize, depth: usize, open: bool },
    Thread { key: String, group: usize, index: usize, depth: usize },
    More { key: String, group: usize, hidden: usize, depth: usize },
    Empty { key: String, group: usize, depth: usize },
}

pub fn activity_of(threads: &[ThreadView]) -> Option<Activity> {
    let has = |a: Activity| {
        threads.iter().any(|t| match a {
            Activity::Unread => t.thread.unread && t.status == Status::Idle,
            Activity::Awaiting => t.status == Status::Awaiting,
            Activity::Running => t.status == Status::Running,
            Activity::Error => t.status == Status::Error,
        })
    };
    [Activity::Awaiting, Activity::Running, Activity::Error, Activity::Unread].into_iter().find(|a| has(*a))
}

const DAY: i64 = 86_400_000;

/// Faixa de data no modo Recentes: `(chave, chave de tradução)`.
pub fn time_bucket(updated: i64, today_start: i64) -> (&'static str, &'static str) {
    if updated >= today_start {
        ("today", "tree.today")
    } else if updated >= today_start - DAY {
        ("yesterday", "tree.yesterday")
    } else if updated >= today_start - 6 * DAY {
        ("week", "tree.last7Days")
    } else if updated >= today_start - 29 * DAY {
        ("month", "tree.last30Days")
    } else {
        ("older", "tree.older")
    }
}

/// Comparação de nomes sem diferenciar maiúsculas, com números em ordem
/// natural (`repo2` antes de `repo10`).
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(c) = x.peek().copied().filter(char::is_ascii_digit) {
                    n.push(c);
                    x.next();
                }
                let mut m = String::new();
                while let Some(d) = y.peek().copied().filter(char::is_ascii_digit) {
                    m.push(d);
                    y.next();
                }
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let o = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

pub struct TreeOpts<'a> {
    pub mode: TreeMode,
    pub filter: &'a str,
    pub archived: bool,
    pub today_start: i64,
}

pub fn build_groups(threads: &[ThreadView], projects: &[Project], o: &TreeOpts) -> Vec<Group> {
    let q = o.filter.trim().to_lowercase();
    let name: HashMap<&str, &str> = projects.iter().map(|p| (p.id.as_str(), p.name.as_str())).collect();
    let mut visible: Vec<ThreadView> = threads
        .iter()
        .filter(|t| t.thread.archived == o.archived)
        .filter(|t| {
            q.is_empty()
                || t.thread.title.to_lowercase().contains(&q)
                || name.get(t.thread.project_id.as_str()).is_some_and(|n| n.to_lowercase().contains(&q))
        })
        .cloned()
        .collect();
    visible.sort_by(|a, b| b.thread.updated_at.cmp(&a.thread.updated_at));

    let mut groups = Vec::new();
    let favs: Vec<ThreadView> = visible.iter().filter(|t| t.thread.bookmarked).cloned().collect();
    if !favs.is_empty() && !o.archived {
        let activity = activity_of(&favs);
        groups.push(Group {
            key: "fav".into(),
            label: "tree.favorites".into(),
            kind: GroupKind::Favorites,
            project: None,
            threads: favs,
            activity,
        });
    }

    if o.mode == TreeMode::Recent {
        let mut buckets: Vec<Group> = Vec::new();
        for t in visible {
            let (k, label) = time_bucket(t.thread.updated_at, o.today_start);
            let key = format!("time:{k}");
            match buckets.iter_mut().find(|g| g.key == key) {
                Some(g) => g.threads.push(t),
                None => {
                    buckets.push(Group { key, label: label.into(), kind: GroupKind::Time, project: None, threads: vec![t], activity: None })
                }
            }
        }
        for mut g in buckets {
            g.activity = activity_of(&g.threads);
            groups.push(g);
        }
        return groups;
    }

    let mut by_project: HashMap<String, Vec<ThreadView>> = HashMap::new();
    for t in visible {
        by_project.entry(t.thread.project_id.clone()).or_default().push(t);
    }
    // todas as pastas do workspace, em ordem alfabética; as sem thread
    // começam recolhidas (ver `is_open`). Filtrando, só entram as que casam.
    let mut shown: Vec<&Project> = projects
        .iter()
        .filter(|p| by_project.contains_key(&p.id) || (!o.archived && (q.is_empty() || p.name.to_lowercase().contains(&q))))
        .collect();
    shown.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    for p in shown {
        let ts = by_project.remove(&p.id).unwrap_or_default();
        let activity = activity_of(&ts);
        groups.push(Group {
            key: format!("p:{}", p.id),
            label: p.name.clone(),
            kind: GroupKind::Project,
            project: Some(p.clone()),
            threads: ts,
            activity,
        });
    }
    groups
}

/// Aberto por padrão: tudo, menos projeto sem thread.
pub fn is_open(g: &Group, collapsed: &HashMap<String, bool>, filtering: bool) -> bool {
    if filtering {
        return true;
    }
    !collapsed.get(&g.key).copied().unwrap_or(g.kind == GroupKind::Project && g.threads.is_empty())
}

pub fn flatten(
    groups: &[Group],
    collapsed: &HashMap<String, bool>,
    show_all: &HashMap<String, bool>,
    per_group: usize,
    filtering: bool,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let open = is_open(g, collapsed, filtering);
        rows.push(Row::Group { key: g.key.clone(), group: gi, depth: 0, open });
        if !open {
            continue;
        }
        if g.threads.is_empty() {
            rows.push(Row::Empty { key: format!("{}:empty", g.key), group: gi, depth: 1 });
            continue;
        }
        let limit = if show_all.get(&g.key).copied().unwrap_or(false) || filtering { g.threads.len() } else { per_group };
        for (i, t) in g.threads.iter().take(limit).enumerate() {
            rows.push(Row::Thread { key: format!("{}/{}", g.key, t.thread.id), group: gi, index: i, depth: 1 });
        }
        if g.threads.len() > limit {
            rows.push(Row::More { key: format!("{}:more", g.key), group: gi, hidden: g.threads.len() - limit, depth: 1 });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Thread;

    fn th(id: &str, project: &str, updated: i64, status: Status) -> ThreadView {
        let t: Thread = serde_json::from_value(serde_json::json!({
            "id": id, "project_id": project, "title": format!("t {id}"), "created_at": 0, "updated_at": updated,
            "cwd": "/x", "isolated": false, "branch": null, "base_ref": null, "session_id": null,
            "model": "default", "permission_mode": "default"
        }))
        .unwrap_or_else(|e| panic!("{e}"));
        ThreadView { thread: t, status }
    }

    fn pr(id: &str, name: &str) -> Project {
        Project { id: id.into(), name: name.into(), path: format!("/{name}"), created_at: 0, is_git: true }
    }

    #[test]
    fn groups_by_project_in_natural_order_with_activity() {
        let threads = vec![th("a", "p1", 5, Status::Idle), th("b", "p2", 9, Status::Awaiting)];
        let projects = vec![pr("p1", "repo10"), pr("p2", "repo2"), pr("p3", "Zeta")];
        let g = build_groups(&threads, &projects, &TreeOpts { mode: TreeMode::Project, filter: "", archived: false, today_start: 0 });
        let labels: Vec<&str> = g.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["repo2", "repo10", "Zeta"]);
        assert_eq!(g.first().and_then(|g| g.activity), Some(Activity::Awaiting));
        let rows = flatten(&g, &HashMap::new(), &HashMap::new(), 10, false);
        // Zeta não tem thread: começa recolhido
        assert!(matches!(rows.last(), Some(Row::Group { open: false, .. })));
    }

    #[test]
    fn recent_mode_buckets_by_day() {
        let today = 40 * DAY;
        let threads = vec![th("a", "p", today + 1, Status::Idle), th("b", "p", today - 1, Status::Idle), th("c", "p", 0, Status::Idle)];
        let g =
            build_groups(&threads, &[pr("p", "x")], &TreeOpts { mode: TreeMode::Recent, filter: "", archived: false, today_start: today });
        let keys: Vec<&str> = g.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, vec!["time:today", "time:yesterday", "time:older"]);
    }

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("repo2", "repo10"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("Api", "api"), std::cmp::Ordering::Equal);
        assert_eq!(natural_cmp("a", "B"), std::cmp::Ordering::Less);
    }
}
