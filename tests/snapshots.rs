//! Snapshot tests for bash-ast
//!
//! These tests parse bash scripts and compare the JSON output against
//! expected snapshots. If no snapshot exists, it creates one.
//!
//! To update snapshots: delete the .expected.json file and run tests.

use bash_ast::{init, parse_to_json};
use std::fs;
use std::path::Path;

fn setup() {
    init();
}

/// Run snapshot tests for all .sh files in the snapshots directory
#[test]
fn test_snapshots() {
    setup();

    let snapshot_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");

    let mut scripts: Vec<_> = fs::read_dir(&snapshot_dir)
        .expect("Failed to read snapshots directory")
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sh"))
        .collect();

    scripts.sort();

    assert!(
        !scripts.is_empty(),
        "No .sh files found in {snapshot_dir:?}"
    );

    let mut failures = Vec::new();
    let mut created = Vec::new();

    for script_path in &scripts {
        let script_name = script_path.file_name().unwrap().to_string_lossy();
        let expected_path = script_path.with_extension("expected.json");

        // Read and parse the script
        let script = fs::read_to_string(script_path)
            .unwrap_or_else(|e| panic!("Failed to read {script_path:?}: {e}"));

        let actual_json = match parse_to_json(&script, true) {
            Ok(json) => json,
            Err(e) => {
                failures.push(format!("{script_name}: parse error: {e}"));
                continue;
            }
        };

        if expected_path.exists() {
            // Compare against expected (normalize to ignore line number differences across platforms)
            let expected_json = fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("Failed to read {expected_path:?}: {e}"));

            let actual_normalized = normalize_json_for_comparison(&actual_json);
            let expected_normalized = normalize_json_for_comparison(&expected_json);

            if actual_normalized != expected_normalized {
                failures.push(format!(
                    "{}: output mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
                    script_name,
                    expected_json.trim(),
                    actual_json.trim()
                ));
            }
        } else {
            // Create the expected file
            fs::write(&expected_path, &actual_json)
                .unwrap_or_else(|e| panic!("Failed to write {expected_path:?}: {e}"));
            created.push(script_name.to_string());
        }
    }

    // Report created files
    if !created.is_empty() {
        println!("\nCreated {} snapshot(s):", created.len());
        for name in &created {
            println!("  - {}.expected.json", name.trim_end_matches(".sh"));
        }
    }

    // Report failures
    assert!(
        failures.is_empty(),
        "\n{} snapshot test(s) failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );

    println!("\nAll {} snapshot tests passed!", scripts.len());
}

/// Verify that we can re-parse the JSON output and get equivalent structure
#[test]
fn test_snapshots_roundtrip() {
    setup();

    let snapshot_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");

    let expected_files: Vec<_> = fs::read_dir(&snapshot_dir)
        .expect("Failed to read snapshots directory")
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();

    for json_path in &expected_files {
        let json = fs::read_to_string(json_path)
            .unwrap_or_else(|e| panic!("Failed to read {json_path:?}: {e}"));

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("{json_path:?} is not valid JSON: {e}"));

        // Verify it has expected structure
        assert!(
            parsed.get("type").is_some(),
            "{json_path:?} missing 'type' field"
        );
    }
}

/// Test full roundtrip: parse -> `to_bash` -> parse -> compare AST
///
/// This verifies that `to_bash` produces valid bash that parses to an equivalent AST.
/// Line numbers are ignored since regenerated code has different formatting.
#[test]
fn test_snapshots_to_bash_roundtrip() {
    use bash_ast::{parse, to_bash};

    setup();

    let snapshot_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");

    // Known failures due to heredoc limitations
    let known_failures: [&str; 0] = [];

    let mut scripts: Vec<_> = fs::read_dir(&snapshot_dir)
        .expect("Failed to read snapshots directory")
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sh"))
        .collect();

    scripts.sort();

    let mut failures = Vec::new();

    for script_path in &scripts {
        let script_name = script_path.file_name().unwrap().to_string_lossy();

        // Skip known failures
        if known_failures.iter().any(|&f| script_name == f) {
            continue;
        }

        let script = fs::read_to_string(script_path)
            .unwrap_or_else(|e| panic!("Failed to read {script_path:?}: {e}"));

        // Parse original
        let ast1 = match parse(&script) {
            Ok(ast) => ast,
            Err(e) => {
                failures.push(format!("{script_name}: failed to parse original: {e}"));
                continue;
            }
        };

        // Convert to bash
        let regenerated = to_bash(&ast1);

        // Parse regenerated
        let ast2 = match parse(&regenerated) {
            Ok(ast) => ast,
            Err(e) => {
                failures.push(format!(
                    "{script_name}: failed to parse regenerated script: {e}\nRegenerated:\n{regenerated}"
                ));
                continue;
            }
        };

        // Compare ASTs (ignoring line numbers)
        let json1 = serde_json::to_string(&ast1).unwrap();
        let json2 = serde_json::to_string(&ast2).unwrap();

        let json1_normalized = normalize_json_for_comparison(&json1);
        let json2_normalized = normalize_json_for_comparison(&json2);

        if json1_normalized != json2_normalized {
            failures.push(format!(
                "{script_name}: AST mismatch after roundtrip\nOriginal AST:\n{json1}\nRegenerated script:\n{regenerated}\nRegenerated AST:\n{json2}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "\n{} roundtrip test(s) failed:\n\n{}",
        failures.len(),
        failures.join("\n\n---\n\n")
    );
}

/// Remove line numbers and sort redirects for comparison
fn normalize_json_for_comparison(json: &str) -> String {
    // Parse and normalize
    let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
    normalize_for_comparison(&mut value);
    serde_json::to_string(&value).unwrap()
}

/// Recursively normalize JSON for comparison
/// - Removes "line" fields
/// - Sorts "redirects" arrays for order-independent comparison
fn normalize_for_comparison(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("line");

            // Sort redirects array by direction+target for consistent ordering
            if let Some(serde_json::Value::Array(redirects)) = map.get_mut("redirects") {
                redirects.sort_by(|a, b| {
                    let key_a = format!(
                        "{}-{}",
                        a.get("direction").and_then(|v| v.as_str()).unwrap_or(""),
                        a.get("target").and_then(|v| v.as_str()).unwrap_or("")
                    );
                    let key_b = format!(
                        "{}-{}",
                        b.get("direction").and_then(|v| v.as_str()).unwrap_or(""),
                        b.get("target").and_then(|v| v.as_str()).unwrap_or("")
                    );
                    key_a.cmp(&key_b)
                });
            }

            for v in map.values_mut() {
                normalize_for_comparison(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                normalize_for_comparison(v);
            }
        }
        _ => {}
    }
}
