use super::model::cosine;
use super::*;

fn lines(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn knowledge_base_is_complete() {
    let issues = &knowledge().issues;
    assert!(issues.len() >= 20);

    let mut ids = std::collections::HashSet::new();
    let known_services = ["*", "caddy", "php-fpm", "mysql", "postgresql"];
    let known_actions = [
        "open_settings",
        "open_logs",
        "open_url",
        "start_php",
        "open_mysql_data",
        "open_postgresql_data",
        "open_runtime",
    ];
    for issue in issues {
        assert!(ids.insert(issue.id.clone()), "duplicate id {}", issue.id);
        assert!(
            !issue.signatures.is_empty(),
            "{} has no signatures",
            issue.id
        );
        assert!(!issue.examples.is_empty(), "{} has no examples", issue.id);
        assert!(issue
            .services
            .iter()
            .all(|s| known_services.contains(&s.as_str())));
        assert!(
            issue.signatures.iter().all(|s| *s == s.to_lowercase()),
            "{}",
            issue.id
        );
        assert!(!issue.title.en.is_empty() && !issue.title.th.is_empty());
        assert!(!issue.cause.en.is_empty() && !issue.cause.th.is_empty());
        assert_eq!(issue.steps.en.len(), issue.steps.th.len(), "{}", issue.id);
        if let Some(action) = &issue.action {
            assert!(known_actions.contains(&action.as_str()), "{}", issue.id);
            assert_eq!(action == "open_url", issue.url.is_some(), "{}", issue.id);
        }
    }
}

#[test]
fn every_example_is_caught_by_its_own_signatures_or_model() {
    // Examples double as regression tests for the signature list: each
    // issue must recognise at least one of its own examples without AI.
    for issue in &knowledge().issues {
        let service = issue
            .services
            .iter()
            .find(|s| s.as_str() != "*")
            .map(String::as_str)
            .unwrap_or("caddy");
        let matches = rank_issues(
            service,
            &lines(
                &issue
                    .examples
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            None,
        );
        assert!(
            matches.iter().any(|m| m.id == issue.id),
            "{} did not match its own examples",
            issue.id
        );
    }
}

#[test]
fn port_conflict_is_detected_from_signature() {
    let matches = rank_issues(
        "caddy",
        &lines(&["Error: loading initial config: listen tcp :8080: bind: address already in use"]),
        None,
    );
    assert_eq!(matches[0].id, "port-in-use");
    assert_eq!(matches[0].method, "signature");
}

#[test]
fn specific_signature_outranks_generic_one() {
    let matches = rank_issues(
        "caddy",
        &lines(&["listen tcp :80: bind: permission denied"]),
        None,
    );
    assert_eq!(matches[0].id, "low-port-permission");
    assert!(matches.iter().any(|m| m.id == "permission-denied"));
}

#[test]
fn service_filter_hides_unrelated_issues() {
    let matches = rank_issues(
        "caddy",
        &lines(&["InnoDB: Unable to lock ./ibdata1 error: 11"]),
        None,
    );
    assert!(matches.iter().all(|m| m.id != "mysql-locked"));
}

#[test]
fn locked_ibdata_outranks_dd_storage_engine_failure() {
    // Windows mysqld reports a data folder held by another mysqld as
    // "must be writable", followed by the generic DD engine failure.
    let matches = rank_issues(
        "mysql",
        &lines(&[
            "[ERROR] [MY-012271] [InnoDB] The innodb_system data file 'ibdata1' must be writable",
            "[ERROR] [MY-010334] [Server] Failed to initialize DD Storage Engine",
            "[ERROR] [MY-010020] [Server] Data Dictionary initialization failed.",
        ]),
        None,
    );
    assert_eq!(matches[0].id, "mysql-locked");
}

#[test]
fn unknown_errors_return_no_matches() {
    let matches = rank_issues("caddy", &lines(&["everything is fine"]), None);
    assert!(matches.is_empty());
}

#[test]
fn candidate_lines_prefer_errors_and_dedupe() {
    let input = lines(&[
        "info: starting",
        "ERROR: port busy",
        "info: retry",
        "ERROR: port busy",
        "fatal: giving up",
    ]);
    assert_eq!(
        candidate_lines(&input),
        (lines(&["ERROR: port busy", "fatal: giving up"]), true)
    );

    let calm = lines(&["one", "two"]);
    assert_eq!(candidate_lines(&calm), (calm, false));
}

#[test]
fn tail_reader_drops_partial_first_line() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("big.log");
    let mut content = "x".repeat(crate::logs::LOG_TAIL_BYTES as usize);
    content.push_str("\nlast line\n");
    fs::write(&path, content).unwrap();

    assert_eq!(
        crate::logs::read_tail_lines(&path).unwrap(),
        lines(&["last line"])
    );
}

/// Manual calibration check against the real model:
/// `CHAMP_DOCTOR_MODEL_DIR=<dir> cargo test doctor -- --ignored --nocapture`
#[test]
#[ignore]
fn real_model_semantic_matching() {
    let Some(dir) = std::env::var_os("CHAMP_DOCTOR_MODEL_DIR") else {
        return;
    };
    let loaded = load_model(Path::new(&dir)).expect("model should load");
    let probes = [
        ("mysql", "Another program has already taken TCP port 3306"),
        (
            "php-fpm",
            "php-cgi.exe - System Error: VCRUNTIME140.dll missing",
        ),
        (
            "postgresql",
            "server files are from an older PostgreSQL release",
        ),
        ("mysql", "the hard drive has no free capacity remaining"),
        (
            "caddy",
            "upstream php backend unreachable, connection was refused",
        ),
        (
            "postgresql",
            "another server instance holds the lock on the data directory",
        ),
        ("caddy", "served 200 GET /index.php in 3ms"),
        (
            "postgresql",
            "database system is ready to accept connections",
        ),
        (
            "mysql",
            "ready for connections. Version: '8.4.3'  port: 3306",
        ),
    ];
    for (service, probe) in probes {
        let Some(vector) = loaded.model.embed(&semantic_text(probe)) else {
            continue;
        };
        let mut scores: Vec<(f32, &str)> = loaded
            .examples
            .iter()
            .map(|(index, example)| {
                (
                    cosine(&vector, example),
                    knowledge().issues[*index].id.as_str(),
                )
            })
            .collect();
        scores.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("{probe}\n  -> {:?}", &scores[..3]);
        let ranked = rank_issues(service, &lines(&[probe]), Some(&loaded));
        println!(
            "  ranked: {:?}",
            ranked
                .iter()
                .map(|m| (m.id.as_str(), m.confidence, m.method))
                .collect::<Vec<_>>()
        );
    }
}
