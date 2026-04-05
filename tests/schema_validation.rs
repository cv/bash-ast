//! Validate parser output against the published JSON Schema.
//!
//! This keeps the schema honest as the AST evolves.

mod common;

use bash_ast::{parse_to_json, schema_json};
use common::{to_bash_regression_scripts, to_bash_roundtrip_matrix_scripts};
use jsonschema::validator_for;
use std::fs;
use std::path::Path;

fn validate_instance(validator: &jsonschema::Validator, instance_json: &str, name: &str) {
    let instance: serde_json::Value = serde_json::from_str(instance_json)
        .unwrap_or_else(|e| panic!("{name}: invalid json fixture: {e}"));

    if let Err(error) = validator.validate(&instance) {
        panic!("{name}: schema validation failed: {error}");
    }
}

#[test]
fn test_schema_accepts_snapshot_fixtures() {
    let schema: serde_json::Value = serde_json::from_str(&schema_json(true)).unwrap();
    let validator = validator_for(&schema).expect("generated schema should be valid");

    let snapshot_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let mut count = 0usize;

    for entry in fs::read_dir(&snapshot_dir).expect("failed to read snapshots directory") {
        let path = entry.expect("failed to read entry").path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let json = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
            validate_instance(&validator, &json, &path.display().to_string());
            count += 1;
        }
    }

    assert!(count > 0, "expected snapshot fixtures");
}

#[test]
fn test_schema_accepts_extended_roundtrip_corpus() {
    let schema: serde_json::Value = serde_json::from_str(&schema_json(true)).unwrap();
    let validator = validator_for(&schema).expect("generated schema should be valid");

    let corpus = to_bash_regression_scripts()
        .iter()
        .chain(to_bash_roundtrip_matrix_scripts().iter());

    let mut count = 0usize;
    for script in corpus {
        let json = parse_to_json(script, false)
            .unwrap_or_else(|e| panic!("failed to parse corpus script {script:?}: {e}"));
        validate_instance(&validator, &json, script);
        count += 1;
    }

    assert!(count > 0, "expected extended corpus fixtures");
}
