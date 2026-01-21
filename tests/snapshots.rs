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
            // Compare against expected
            let expected_json = fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("Failed to read {expected_path:?}: {e}"));

            if actual_json.trim() != expected_json.trim() {
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
