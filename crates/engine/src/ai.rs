//! IA de commit: mensagem de commit e divisão do staging em commits. Roda
//! pelo Claude Code do workspace (`claude -p`, ver `claude::complete`), com a
//! conta dele: o app não guarda chave de API.

use std::path::Path;

use serde::Serialize;

use lazyaicode_git::error::GitCommandError;

const DEFAULT_COMMIT_PROMPT: &str = r#"You are an expert AI assistant that generates Git commit messages. Your **sole purpose** is to create messages that **strictly adhere** to the Conventional Commits specification, with two important modifications:
1.  You **MUST NOT** use the optional `scope` element.
2.  The description in the header **MUST NOT** end with a period.

Given a set of code changes (a diff), you will generate a single, complete commit message.

---

## Core Rules

-   **Structure**: Your generated message MUST follow this structure:
    ```
    <type>[!]: <description>

    [optional body]

    [optional footer(s)]
    ```
-   **No Scope**: You **MUST NOT** include a scope in the header.
    -   **Correct:** `feat: add user login`
    -   **Incorrect:** `feat(auth): add user login`
-   **Header is Mandatory**: The first line (`<type>[!]: <description>`) is always required.
-   **Blank Lines**: Use a single blank line to separate the header from the body, and the body from the footer(s).

---

## Message Components

### 1. Header

The header is the first line and consists of three parts:

#### Type
MUST be one of the following lowercase keywords:
-   **`feat`**: A new feature is introduced.
-   **`fix`**: A bug fix.
-   **`build`**: Changes that affect the build system or external dependencies (e.g., gulp, npm).
-   **`chore`**: Other changes that don't modify source or test files (e.g., updating dependencies).
-   **`ci`**: Changes to our CI configuration files and scripts (e.g., Travis, Circle).
-   **`docs`**: Documentation only changes.
-   **`perf`**: A code change that improves performance.
-   **`refactor`**: A code change that neither fixes a bug nor adds a feature.
-   **`style`**: Changes that do not affect the meaning of the code (white-space, formatting, etc).
-   **`test`**: Adding missing tests or correcting existing tests.

#### Breaking Change Indicator (`!`) (Optional)
If the commit introduces a breaking API change, append a `!` immediately after the `type`. This is mandatory for breaking changes.

#### Description
A short, concise summary of the code changes.
-   Start immediately after the colon and space.
-   Use the imperative mood (e.g., "add", "fix", "change" not "added", "fixes", "changed").
-   Do not capitalize the first letter.
-   **Do not end with a period.**

### 2. Body (Optional)

-   Provide additional context after the header, separated by a blank line.
-   Explain the "why" and "how" of the change, not just the "what".
-   May consist of multiple paragraphs separated by blank lines.

### 3. Footer(s) (Optional)

-   Appears after the body, separated by a blank line.
-   Used for tracking metadata. Each footer consists of a token, a separator (`: ` or ` #`), and a value.
-   **BREAKING CHANGE Footer**: This is the most important footer.
    -   It MUST start with `BREAKING CHANGE: ` (with the space) in all uppercase.
    -   It is **REQUIRED** if the commit introduces a breaking change. It can be used in addition to the `!` in the header for clarity.
    -   The text after the colon should explain the breaking change, its impact, and any migration instructions.
-   Another common footer is `Refs: #123` to link to issue numbers.

---

## Examples

#### Simple Fix
```
fix: correct user authentication token refresh logic
```

#### New Feature with Body
```
feat: implement user profile page

Users can now view and edit their profile information,
including username, email, and bio. This closes the loop
on the initial user management implementation.
```

#### Breaking Change with `!` and a Body
```
refactor!: rename user ID field from `uid` to `userId`

This change standardizes the user identification field across the entire
API to improve consistency. All client applications must be updated to
reference `userId` instead of `uid`.
```

#### Breaking Change with `!` and a Footer
```
chore!: drop support for Node 14

BREAKING CHANGE: The minimum required Node.js version is now 16.
The codebase has been updated to use features, such as the optional
chaining operator, that are not available in Node 14.
```

#### Commit with Body and Footer
```
fix: prevent race conditions on data fetching

Introduce a request ID to track the latest data request.
Incoming responses are now ignored if they do not match the
ID of the most recent request.

This completely replaces the previous timeout-based mitigation strategy.

Refs: #451
```

Make sure to not add backticks (```), quotes or anything related at start and end of the commit message"#;

const COMPOSE_PROMPT: &str = r#"You are an expert AI assistant that splits a set of STAGED changes into a small series of atomic Git commits.

You receive the staged changes cut into numbered **units**. Each unit has an id in square brackets and its diff: either one hunk of a modified file (`path#N`, zero-context, so one block of changed lines) or a whole file (`path`: added, deleted, binary...). On a second round you also receive the user's answers to your questions.

## Splitting rules

-   Group units by **feature / purpose**: one commit per logical change (a feature, a fix, a refactor, docs, build/config...). Do NOT create one commit per unit or per file, and do NOT lump unrelated work together.
-   A single file can hold several features: assign its hunks (`path#0`, `path#1`...) to different commits when they belong to different features. Read the hunks, not just the file names.
-   Aim for the **fewest commits that stay atomic**. A change that is tiny or only exists to support another change belongs in that other change's commit.
-   Each commit must be **self-contained and cherry-pickable**: it should make sense on its own, and must not depend on a later commit in the series to compile or work. When one change depends on another, put the dependency first and order the commits accordingly.
-   Every unit MUST appear in **exactly one** commit. Use the unit ids exactly as given and never invent ids.
-   Order the commits so that the series builds up logically (foundations first).

## When in doubt, ask

If the grouping is genuinely ambiguous (e.g. you cannot tell whether two changes are related, or a file's purpose is unclear), do NOT guess: return questions instead of commits. Ask only what you really need, as few and as short as possible.

## Commit messages

Every commit's `summary` and `description` must follow the commit message rules below. `summary` is the header line (`<type>[!]: <description>`), `description` is the body/footers (empty string if none).

## Output format (overrides any formatting instruction in the rules below)

Respond with **only** a JSON object, no markdown fences, no extra text. Exactly one of these shapes:

{"commits":[{"summary":"feat: ...","description":"...","changes":["path/a#0","path/b"]}]}

{"questions":["...","..."]}
"#;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedCommit {
    pub summary: String,
    pub description: String,
    /// Unit ids (see `git::compose`).
    pub changes: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComposeResult {
    pub commits: Vec<ComposedCommit>,
    pub questions: Vec<String>,
    /// Index tree the plan was made for; passed back when applying it.
    pub tree_id: String,
}

#[derive(serde::Deserialize, Default)]
struct RawCompose {
    #[serde(default)]
    commits: Vec<RawCommit>,
    #[serde(default)]
    questions: Vec<String>,
}

#[derive(serde::Deserialize)]
struct RawCommit {
    summary: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    changes: Vec<String>,
}

/// Model output → `ComposeResult`, tolerating stray fences/prose around the
/// JSON object. Checks that every staged file is assigned exactly once.
fn parse_compose(raw: &str, staged_ids: &[String]) -> Result<ComposeResult, GitCommandError> {
    let bad = |m: &str| GitCommandError::Internal(format!("Composição com IA: {m}"));
    let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) else {
        return Err(bad("a resposta não era JSON"));
    };
    let parsed: RawCompose = serde_json::from_str(&raw[start..=end]).map_err(|e| bad(&format!("JSON inválido ({e})")))?;
    if !parsed.questions.is_empty() && parsed.commits.is_empty() {
        return Ok(ComposeResult { questions: parsed.questions, ..Default::default() });
    }
    let mut seen = std::collections::HashSet::new();
    for f in parsed.commits.iter().flat_map(|c| &c.changes) {
        if !staged_ids.contains(f) {
            return Err(bad(&format!("alteração desconhecida \"{f}\"")));
        }
        if !seen.insert(f) {
            return Err(bad(&format!("alteração \"{f}\" atribuída a mais de um commit")));
        }
    }
    if let Some(missing) = staged_ids.iter().find(|f| !seen.contains(f)) {
        return Err(bad(&format!("alteração \"{missing}\" não foi atribuída a nenhum commit")));
    }
    if parsed.commits.iter().any(|c| c.summary.trim().is_empty() || c.changes.is_empty()) {
        return Err(bad("um commit veio sem resumo ou sem alterações"));
    }
    Ok(ComposeResult {
        commits: parsed
            .commits
            .into_iter()
            .map(|c| ComposedCommit {
                summary: c.summary.trim().to_string(),
                description: c.description.trim().to_string(),
                changes: c.changes,
            })
            .collect(),
        questions: Vec::new(),
        tree_id: String::new(),
    })
}

/// `units` is `(id, text)` per staged change unit; `answers` holds the
/// user's replies to an earlier round of questions (empty on the first call).
pub async fn compose_commits(
    claude_dir: &Path,
    model: &str,
    units: &[(String, String)],
    custom_instructions: &str,
    answers: &str,
) -> Result<ComposeResult, GitCommandError> {
    let system_prompt = format!("{COMPOSE_PROMPT}\n---\n\n# Commit message rules\n\n{}", build_system_prompt(custom_instructions));

    let mut input = String::from("Staged change units:\n");
    for (id, text) in units {
        input.push_str(&format!("\n### [{id}]\n{text}\n"));
    }
    if !answers.trim().is_empty() {
        input.push_str(&format!("\nUser's answers to your questions:\n{answers}\n"));
    }

    let raw = complete(claude_dir, model, &system_prompt, &input).await?;
    let ids: Vec<String> = units.iter().map(|(id, _)| id.clone()).collect();
    parse_compose(&raw, &ids)
}

async fn complete(claude_dir: &Path, model: &str, system_prompt: &str, input: &str) -> Result<String, GitCommandError> {
    lazyaicode_agent::claude::complete(claude_dir, model, system_prompt, input).await.map_err(|e| GitCommandError::Internal(e.to_string()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessageResult {
    pub summary: String,
    pub description: String,
}

fn build_system_prompt(custom_instructions: &str) -> String {
    let custom_instructions = custom_instructions.trim();
    if custom_instructions.is_empty() {
        return DEFAULT_COMMIT_PROMPT.to_string();
    }
    format!(
        "{DEFAULT_COMMIT_PROMPT}\n\n---\n\nAdditional instructions provided by the user. These take \
         priority over the rules above whenever they conflict, but should be treated as a supplement \
         to them, not a replacement:\n\n{custom_instructions}"
    )
}

fn split_commit_message(raw: &str) -> (String, String) {
    let trimmed = raw.trim().trim_matches('`').trim_matches('"').trim();
    let mut lines = trimmed.splitn(2, '\n');
    let summary = lines.next().unwrap_or_default().trim().to_string();
    let description = lines.next().unwrap_or_default().trim().to_string();
    (summary, description)
}

pub async fn generate_commit_message(
    claude_dir: &Path,
    model: &str,
    diff: &str,
    custom_instructions: &str,
) -> Result<CommitMessageResult, GitCommandError> {
    let raw = complete(claude_dir, model, &build_system_prompt(custom_instructions), diff).await?;
    let (summary, description) = split_commit_message(&raw);
    Ok(CommitMessageResult { summary, description })
}

const CONFLICT_PROMPT: &str = r#"You resolve Git merge conflicts. You receive one file in three versions: BASE (common ancestor, may be absent), OURS (the current branch) and THEIRS (the incoming branch).

Produce the final merged file:
- Keep every change from both sides that is compatible; when the two changes truly contradict, choose the one that makes the code correct and consistent with the rest of the file.
- Never invent code that is in neither side, and never drop unrelated content.
- Preserve the file's indentation, line endings and trailing newline.

Reply with ONLY the complete resolved file content: no explanations, no code fences, and no conflict markers (<<<<<<<, =======, >>>>>>>)."#;

/// Largest total size of the three versions sent to the model.
const MAX_CONFLICT_CHARS: usize = 200_000;

/// The model's reply as file content: drops a wrapping code fence, rejects an
/// empty reply or one that still has conflict markers.
pub fn clean_resolution(raw: &str) -> Result<String, GitCommandError> {
    let mut text = raw.trim_end_matches(['\r', '\n']).to_string();
    let trimmed = text.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") && trimmed.lines().count() >= 2 {
        let inner: Vec<&str> = trimmed.lines().collect();
        text = inner[1..inner.len() - 1].join("\n");
    }
    if text.trim().is_empty() {
        return Err(GitCommandError::Internal("o modelo devolveu uma resolução vazia".into()));
    }
    if text.lines().any(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>") || l == "=======") {
        return Err(GitCommandError::Internal("o modelo deixou marcadores de conflito na resolução".into()));
    }
    Ok(text + "\n")
}

fn conflict_input(path: &str, base: Option<&str>, ours: &str, theirs: &str) -> String {
    format!("File: {path}\n\n=== BASE ===\n{}\n=== OURS ===\n{ours}\n=== THEIRS ===\n{theirs}\n", base.unwrap_or("(no common ancestor)"))
}

pub async fn resolve_conflict(
    claude_dir: &Path,
    model: &str,
    path: &str,
    base: Option<&str>,
    ours: &str,
    theirs: &str,
    custom_instructions: &str,
) -> Result<String, GitCommandError> {
    let input = conflict_input(path, base, ours, theirs);
    if input.len() > MAX_CONFLICT_CHARS {
        return Err(GitCommandError::Internal("arquivo grande demais para resolver com IA".into()));
    }
    let custom = custom_instructions.trim();
    let system_prompt = if custom.is_empty() {
        CONFLICT_PROMPT.to_string()
    } else {
        format!("{CONFLICT_PROMPT}\n\nAdditional instructions from the user:\n{custom}")
    };
    clean_resolution(&complete(claude_dir, model, &system_prompt, &input).await?)
}

const PR_PROMPT: &str = r#"You write pull request titles and descriptions. You receive the list of commits and the combined diff of a branch.

Reply with exactly:
- Line 1: the PR title — short, imperative, no trailing period, no prefix like "Title:".
- A blank line.
- The description in Markdown: a one-paragraph summary of what and why, then a bullet list of the notable changes. Mention breaking changes or required follow-up only if the diff shows them.

Base everything on the commits and diff; do not invent features, tests or issue numbers. No code fences around the reply."#;

/// (title, description) from the model's reply, tolerating a markdown heading
/// or "Title:" prefix on the first line.
pub fn split_pr(raw: &str) -> (String, String) {
    let (first, rest) = split_commit_message(raw);
    let title = first.trim_start_matches('#').trim().trim_start_matches("Title:").trim().trim_matches('*').trim().to_string();
    (title, rest)
}

pub async fn generate_pr_description(
    claude_dir: &Path,
    model: &str,
    commits: &[String],
    diff: &str,
) -> Result<CommitMessageResult, GitCommandError> {
    let list = commits.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n");
    let input = format!("Commits:\n{list}\n\nDiff:\n{diff}");
    let raw = complete(claude_dir, model, PR_PROMPT, &input).await?;
    let (summary, description) = split_pr(&raw);
    if summary.is_empty() {
        return Err(GitCommandError::Internal("o modelo devolveu um título vazio".into()));
    }
    Ok(CommitMessageResult { summary, description })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<String> {
        vec!["a.rs".into(), "b.rs".into()]
    }

    #[test]
    fn parses_commits_and_questions() {
        let ok = r#"```json
{"commits":[{"summary":"feat: x","description":"","changes":["a.rs"]},{"summary":"fix: y","changes":["b.rs"]}]}
```"#;
        assert_eq!(parse_compose(ok, &files()).unwrap().commits.len(), 2);
        let q = parse_compose(r#"{"questions":["why?"]}"#, &files()).unwrap();
        assert_eq!(q.questions, vec!["why?"]);
    }

    #[test]
    fn rejects_bad_file_assignment() {
        let missing = r#"{"commits":[{"summary":"feat: x","changes":["a.rs"]}]}"#;
        assert!(parse_compose(missing, &files()).is_err());
        let dup = r#"{"commits":[{"summary":"a","changes":["a.rs","b.rs"]},{"summary":"b","changes":["a.rs"]}]}"#;
        assert!(parse_compose(dup, &files()).is_err());
        let unknown = r#"{"commits":[{"summary":"a","changes":["a.rs","b.rs","c.rs"]}]}"#;
        assert!(parse_compose(unknown, &files()).is_err());
    }

    #[test]
    fn cleans_the_model_resolution() {
        assert_eq!(clean_resolution("a\nb\n\n").unwrap(), "a\nb\n");
        assert_eq!(clean_resolution("```rust\nfn x() {}\n```").unwrap(), "fn x() {}\n");
        assert!(clean_resolution("  \n").is_err());
        assert!(clean_resolution("a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> x\n").is_err());
    }

    #[test]
    fn conflict_input_lists_all_three_versions() {
        let i = conflict_input("f.rs", None, "o", "t");
        assert!(i.contains("(no common ancestor)") && i.contains("=== OURS ===\no") && i.contains("=== THEIRS ===\nt"));
    }

    #[test]
    fn splits_the_pr_reply_into_title_and_body() {
        let (t, b) = split_pr("Add login\n\nSummary.\n\n- one\n- two");
        assert_eq!((t.as_str(), b.as_str()), ("Add login", "Summary.\n\n- one\n- two"));
        assert_eq!(split_pr("# Title: **Fix crash**\n\nbody").0, "Fix crash");
        assert_eq!(split_pr("Only a title").1, "");
    }
}
