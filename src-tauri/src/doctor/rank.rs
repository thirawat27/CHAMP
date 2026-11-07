//! Matching log lines against the knowledge base and the embedding model.

use super::knowledge::knowledge;
use super::model::cosine;
use super::{DiagnosisMatch, LoadedModel};

/// Error-looking lines kept for matching (most recent last).
pub(super) const MAX_CANDIDATE_LINES: usize = 40;

/// Lines shown back to the user as evidence.
pub(super) const MAX_EXCERPT_LINES: usize = 25;

pub(super) const MAX_LINE_CHARS: usize = 400;

pub(super) const MAX_MATCHES: usize = 3;

/// Minimum confidence for a match to be reported.
pub(super) const MIN_CONFIDENCE: f32 = 0.6;

/// Cosine similarity at which a semantic match starts to count, and the
/// similarity treated as a certain match. Calibrated on potion-base-8M (see
/// `real_model_semantic_matching`): reworded lines describing the same failure
/// score ~0.62-0.78, neighbouring-but-wrong issues ~0.50-0.55.
pub(super) const AI_SIMILARITY_FLOOR: f32 = 0.58;

pub(super) const AI_SIMILARITY_CEILING: f32 = 0.8;

/// Semantic matches stay below exact signature matches.
pub(super) const AI_MAX_CONFIDENCE: f32 = 0.85;

pub(super) const ERROR_HINTS: [&str; 24] = [
    "error",
    "fail",
    "fatal",
    "warn",
    "denied",
    "cannot",
    "can't",
    "could not",
    "couldn't",
    "unable",
    "refused",
    "not found",
    "exception",
    "panic",
    "abort",
    "corrupt",
    "exited",
    "invalid",
    "missing",
    "no space",
    "in use",
    "not permitted",
    "killed",
    "exhausted",
];

/// Keep recent, error-looking lines (deduplicated, most recent last).
///
/// The flag is false when no line looked like an error and the result is
/// just the log's last lines, shown for context.
pub(super) fn candidate_lines(lines: &[String]) -> (Vec<String>, bool) {
    let clean = |line: &str| -> Option<String> {
        let trimmed = line.trim();
        (!trimmed.is_empty()).then(|| trimmed.chars().take(MAX_LINE_CHARS).collect())
    };

    let mut selected: Vec<String> = lines
        .iter()
        .filter(|line| {
            let lower = line.to_lowercase();
            ERROR_HINTS.iter().any(|hint| lower.contains(hint))
        })
        .filter_map(|line| clean(line))
        .collect();

    let has_errors = !selected.is_empty();
    if !has_errors {
        selected = lines.iter().filter_map(|line| clean(line)).collect();
        let keep_from = selected.len().saturating_sub(15);
        selected.drain(..keep_from);
    }

    // Deduplicate, keeping the most recent occurrence of each line.
    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = selected
        .into_iter()
        .rev()
        .filter(|line| seen.insert(line.clone()))
        .collect();
    unique.truncate(MAX_CANDIDATE_LINES);
    unique.reverse();
    (unique, has_errors)
}

/// Words that appear in almost every candidate line (service names, log
/// levels) carry no meaning for matching and would pull every line toward the
/// same issues, so they are removed before embedding.
pub(super) const SEMANTIC_STOPWORDS: [&str; 16] = [
    "mysql",
    "mysqld",
    "postgresql",
    "postgres",
    "php",
    "fpm",
    "cgi",
    "caddy",
    "champ",
    "innodb",
    "error",
    "fatal",
    "warning",
    "note",
    "info",
    "level",
];

/// Prepare text for embedding: drop paths/URLs, numbers, timestamps, and stopwords.
pub(super) fn semantic_text(text: &str) -> String {
    text.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '='
            )
    })
    .filter(|word| !word.contains('/') && !word.contains('\\'))
    .flat_map(|word| word.split(['-', '_', '.', ':']))
    .map(|word| {
        word.trim_matches(|ch: char| !ch.is_alphanumeric())
            .to_lowercase()
    })
    .filter(|word| {
        word.chars().any(char::is_alphabetic) && !SEMANTIC_STOPWORDS.contains(&word.as_str())
    })
    .collect::<Vec<_>>()
    .join(" ")
}

/// Words of a bare "process exited" line. Such a line says that a service
/// stopped, not why, so it has no meaning to match on.
const EXIT_ONLY_WORDS: [&str; 9] = [
    "exited",
    "during",
    "start",
    "unexpectedly",
    "process",
    "exit",
    "code",
    "status",
    "with",
];

/// True for lines the service itself marks as a warning. Healthy services log
/// these all the time (MySQL's empty-password note on first start, Caddy's
/// "Caddyfile input is not formatted"), so a similar-sounding issue must not be
/// proposed from them. Known problems are still caught by exact signatures.
pub(super) fn is_warning_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    ["[warning]", "\"level\":\"warn\"", "php warning:", "[warn]"]
        .iter()
        .any(|marker| lower.contains(marker))
}

/// True when text prepared by [`semantic_text`] only reports that a process exited.
pub(super) fn names_no_cause(semantic: &str) -> bool {
    semantic
        .split_whitespace()
        .all(|word| EXIT_ONLY_WORDS.contains(&word))
}

pub(super) fn signature_confidence(signature: &str) -> f32 {
    // Longer signatures are more specific, so they win ties against short,
    // generic ones (e.g. "listen tcp :80: bind: permission denied" beats
    // "permission denied").
    0.86 + (signature.len() as f32 / 400.0).min(0.12)
}

pub(super) fn ai_confidence(similarity: f32) -> f32 {
    let scaled = (similarity - AI_SIMILARITY_FLOOR) / (AI_SIMILARITY_CEILING - AI_SIMILARITY_FLOOR);
    (MIN_CONFIDENCE + scaled * (AI_MAX_CONFIDENCE - MIN_CONFIDENCE)).min(AI_MAX_CONFIDENCE)
}

/// Rank knowledge-base issues against candidate log lines (most recent last).
pub(super) fn rank_issues(
    service: &str,
    lines: &[String],
    model: Option<&LoadedModel>,
) -> Vec<DiagnosisMatch> {
    let issues = &knowledge().issues;
    // (confidence, evidence, method) per issue
    let mut best: Vec<Option<(f32, String, &'static str)>> = vec![None; issues.len()];
    let mut consider = |index: usize, confidence: f32, evidence: &str, method: &'static str| {
        let replace = match &best[index] {
            Some((current, _, _)) => confidence > *current,
            None => true,
        };
        if replace {
            best[index] = Some((confidence, evidence.to_string(), method));
        }
    };

    for line in lines.iter().rev() {
        let lower = line.to_lowercase();
        for (index, issue) in issues.iter().enumerate() {
            if !issue.applies_to(service) {
                continue;
            }
            if let Some(signature) = issue
                .signatures
                .iter()
                .filter(|signature| lower.contains(signature.as_str()))
                .max_by_key(|signature| signature.len())
            {
                consider(index, signature_confidence(signature), line, "signature");
            }
        }
    }

    if let Some(loaded) = model {
        // A static embedding model separates "related" from "unrelated" well,
        // but neighbouring issues of one service score close together. Only the
        // single most similar issue is proposed; signatures can add more.
        let mut top: Option<(f32, usize, &String)> = None;
        for line in lines.iter().rev() {
            if is_warning_line(line) {
                continue;
            }
            let text = semantic_text(line);
            if names_no_cause(&text) {
                continue;
            }
            let Some(vector) = loaded.model.embed(&text) else {
                continue;
            };
            for (index, example) in &loaded.examples {
                if !issues[*index].applies_to(service) {
                    continue;
                }
                let similarity = cosine(&vector, example);
                if top.is_none_or(|(current, _, _)| similarity > current) {
                    top = Some((similarity, *index, line));
                }
            }
        }
        if let Some((similarity, index, line)) = top {
            if similarity >= AI_SIMILARITY_FLOOR {
                consider(index, ai_confidence(similarity), line, "ai");
            }
        }
    }

    let mut matches: Vec<DiagnosisMatch> = best
        .into_iter()
        .enumerate()
        .filter_map(|(index, scored)| {
            let (confidence, evidence, method) = scored?;
            if confidence < MIN_CONFIDENCE {
                return None;
            }
            let issue = &issues[index];
            Some(DiagnosisMatch {
                id: issue.id.clone(),
                title: issue.title.clone(),
                cause: issue.cause.clone(),
                steps: issue.steps.clone(),
                confidence: (confidence * 100.0).round() / 100.0,
                evidence,
                method,
                action: issue.action.clone(),
                url: issue.url.clone(),
            })
        })
        .collect();

    matches.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    matches.truncate(MAX_MATCHES);
    matches
}
