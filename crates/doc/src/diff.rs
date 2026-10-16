//! Linhas de um patch unificado (`git diff`) com a numeração dos dois lados.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Hunk,
    Ctx,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// `@@ -a[,n] +b[,m] @@…` → `(a, b)`.
fn hunk_start(line: &str) -> Option<(u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let first = |s: &str| s.split(',').next().and_then(|n| n.parse::<u32>().ok());
    Some((first(old)?, first(new)?))
}

pub fn parse_patch(patch: &str) -> Vec<DiffLine> {
    let mut out = Vec::new();
    let (mut a, mut b, mut in_hunk) = (0u32, 0u32, false);
    for line in patch.split('\n') {
        if let Some((x, y)) = hunk_start(line) {
            (a, b, in_hunk) = (x, y, true);
            out.push(DiffLine { kind: LineKind::Hunk, text: line.to_string(), old: None, new: None });
            continue;
        }
        if !in_hunk || line.starts_with('\\') {
            continue;
        }
        let body = line.get(1..).unwrap_or("").to_string();
        let l = match line.as_bytes().first() {
            Some(b'+') => {
                b = b.saturating_add(1);
                DiffLine { kind: LineKind::Add, text: body, old: None, new: Some(b - 1) }
            }
            Some(b'-') => {
                a = a.saturating_add(1);
                DiffLine { kind: LineKind::Del, text: body, old: Some(a - 1), new: None }
            }
            _ => {
                a = a.saturating_add(1);
                b = b.saturating_add(1);
                DiffLine { kind: LineKind::Ctx, text: body, old: Some(a - 1), new: Some(b - 1) }
            }
        };
        out.push(l);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_both_sides() {
        let p = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@ fn\n a\n-b\n+c\n\\ No newline at end of file";
        let l = parse_patch(p);
        assert_eq!(l.len(), 4);
        assert_eq!((l[1].old, l[1].new), (Some(1), Some(1)));
        assert_eq!((l[2].kind, l[2].old), (LineKind::Del, Some(2)));
        assert_eq!((l[3].kind, l[3].new), (LineKind::Add, Some(2)));
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "(@@ -[0-9]{0,12},?[0-9]{0,3} \\+[0-9]{0,12} @@|[+ -\\\\].{0,8}|\n){0,30}") {
            let _ = parse_patch(&s);
        }
    }
}
