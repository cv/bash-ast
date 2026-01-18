//! Benchmarks for bash-ast parsing performance
//!
//! Run with: cargo bench
//!
//! Results are saved to target/criterion/ with HTML reports.

use bash_ast::{init, parse, parse_to_json};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Once;

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        init();
    });
}

// ============================================================================
// Simple Command Benchmarks
// ============================================================================

fn bench_simple_command(c: &mut Criterion) {
    setup();

    let mut group = c.benchmark_group("simple_commands");

    // Minimal command
    group.bench_function("echo_hello", |b| b.iter(|| parse(black_box("echo hello"))));

    // Command with multiple arguments
    group.bench_function("echo_many_args", |b| {
        b.iter(|| {
            parse(black_box(
                "echo one two three four five six seven eight nine ten",
            ))
        })
    });

    // Command with quoted strings
    group.bench_function("echo_quoted", |b| {
        b.iter(|| parse(black_box(r#"echo "hello world" 'foo bar'"#)))
    });

    // Command with variables
    group.bench_function("echo_variables", |b| {
        b.iter(|| parse(black_box("echo $HOME $PATH $USER $SHELL")))
    });

    group.finish();
}

// ============================================================================
// Pipeline Benchmarks
// ============================================================================

fn bench_pipelines(c: &mut Criterion) {
    setup();

    let mut group = c.benchmark_group("pipelines");

    // Simple 2-stage pipeline
    group.bench_function("pipe_2", |b| {
        b.iter(|| parse(black_box("cat file | grep pattern")))
    });

    // 5-stage pipeline
    group.bench_function("pipe_5", |b| {
        b.iter(|| {
            parse(black_box(
                "cat file | grep pattern | sort | uniq | head -10",
            ))
        })
    });

    // Vary pipeline length
    for n in [2, 5, 10, 20].iter() {
        let commands: Vec<&str> = (0..*n).map(|_| "cat").collect();
        let script = commands.join(" | ");

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::new("pipe_n", n), &script, |b, script| {
            b.iter(|| parse(black_box(script)))
        });
    }

    group.finish();
}

// ============================================================================
// Control Flow Benchmarks
// ============================================================================

fn bench_control_flow(c: &mut Criterion) {
    setup();

    let mut group = c.benchmark_group("control_flow");

    // Simple if statement
    group.bench_function("if_simple", |b| {
        b.iter(|| parse(black_box("if true; then echo yes; fi")))
    });

    // If-else statement
    group.bench_function("if_else", |b| {
        b.iter(|| parse(black_box("if true; then echo yes; else echo no; fi")))
    });

    // If-elif-else statement
    group.bench_function("if_elif_else", |b| {
        b.iter(|| {
            parse(black_box(
                "if test1; then cmd1; elif test2; then cmd2; else cmd3; fi",
            ))
        })
    });

    // For loop
    group.bench_function("for_loop", |b| {
        b.iter(|| parse(black_box("for i in a b c d e; do echo $i; done")))
    });

    // While loop
    group.bench_function("while_loop", |b| {
        b.iter(|| parse(black_box("while true; do echo loop; done")))
    });

    // Case statement
    group.bench_function("case_stmt", |b| {
        b.iter(|| {
            parse(black_box(
                "case $x in a) echo a;; b) echo b;; *) echo default;; esac",
            ))
        })
    });

    group.finish();
}

// ============================================================================
// Complex Script Benchmarks
// ============================================================================

fn bench_complex_scripts(c: &mut Criterion) {
    setup();

    let mut group = c.benchmark_group("complex_scripts");

    // Nested structures
    group.bench_function("nested_if_for", |b| {
        b.iter(|| {
            parse(black_box(
                "if true; then for i in 1 2 3; do while true; do echo $i; break; done; done; fi",
            ))
        })
    });

    // Function definition
    group.bench_function("function_def", |b| {
        b.iter(|| parse(black_box("myfunc() { echo hello; return 0; }")))
    });

    // Script with redirections
    group.bench_function("redirections", |b| {
        b.iter(|| parse(black_box("cmd < input.txt > output.txt 2>&1")))
    });

    // Command substitution
    group.bench_function("command_substitution", |b| {
        b.iter(|| parse(black_box("echo $(date) `whoami` $((1+2))")))
    });

    // Realistic script fragment
    let realistic_script = r#"
for file in *.txt; do
    if [[ -f "$file" ]]; then
        cat "$file" | grep pattern || echo "No match"
    fi
done
"#
    .trim();

    group.bench_function("realistic_script", |b| {
        b.iter(|| parse(black_box(realistic_script)))
    });

    group.finish();
}

// ============================================================================
// Scaling Benchmarks
// ============================================================================

fn bench_scaling(c: &mut Criterion) {
    setup();

    let mut group = c.benchmark_group("scaling");
    group.sample_size(50); // Reduce sample size for larger inputs

    // Scale by argument count
    for n in [10, 100, 500, 1000].iter() {
        let args: Vec<String> = (0..*n).map(|i| format!("arg{}", i)).collect();
        let script = format!("echo {}", args.join(" "));
        let bytes = script.len();

        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(BenchmarkId::new("args", n), &script, |b, script| {
            b.iter(|| parse(black_box(script)))
        });
    }

    // Scale by nesting depth
    for depth in [5, 10, 20, 50].iter() {
        let mut script = String::new();
        for _ in 0..*depth {
            script.push_str("{ ");
        }
        script.push_str("echo x; ");
        for _ in 0..*depth {
            script.push_str("}; ");
        }
        script.pop();
        script.pop();

        group.bench_with_input(BenchmarkId::new("nesting", depth), &script, |b, script| {
            b.iter(|| parse(black_box(script)))
        });
    }

    group.finish();
}

// ============================================================================
// JSON Output Benchmarks
// ============================================================================

fn bench_json_output(c: &mut Criterion) {
    setup();

    let mut group = c.benchmark_group("json_output");

    let script = "for i in a b c; do echo $i; done | grep a";

    // Parse only (baseline)
    group.bench_function("parse_only", |b| b.iter(|| parse(black_box(script))));

    // Parse + JSON (compact)
    group.bench_function("parse_json_compact", |b| {
        b.iter(|| parse_to_json(black_box(script), false))
    });

    // Parse + JSON (pretty)
    group.bench_function("parse_json_pretty", |b| {
        b.iter(|| parse_to_json(black_box(script), true))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_simple_command,
    bench_pipelines,
    bench_control_flow,
    bench_complex_scripts,
    bench_scaling,
    bench_json_output,
);
criterion_main!(benches);
