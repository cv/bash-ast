// build.rs - Build script for bash-ast
//
// This script:
// 1. Configures and builds GNU Bash if needed
// 2. Generates Rust FFI bindings using bindgen
// 3. Links the bash static library

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bash_src = manifest_dir.join("bash");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=safe_parse.c");
    println!("cargo:rerun-if-changed=bash/");

    // Step 1: Configure and build bash if not already done
    if !bash_src.join("config.h").exists() {
        configure_bash(&bash_src);
    }

    if !bash_src.join("y.tab.o").exists() {
        build_bash(&bash_src);
    }

    // Step 2: Compile our safe_parse.c wrapper
    compile_safe_parse(&manifest_dir, &bash_src, &out_dir);

    // Step 3: Create static library from bash objects (including safe_parse.o)
    create_static_library(&bash_src, &out_dir);

    // Step 4: Generate bindings
    generate_bindings(&bash_src, &out_dir);

    // Step 5: Link libraries
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=bash_parser");
    println!("cargo:rustc-link-lib=ncurses");

    // Link optional bash libraries if present
    let lib_dir = bash_src.join("lib");
    for (subdir, libname) in [("malloc", "malloc"), ("intl", "intl")] {
        let lib_path = lib_dir.join(subdir).join(format!("lib{libname}.a"));
        if lib_path.exists() {
            println!(
                "cargo:rustc-link-search=native={}",
                lib_dir.join(subdir).display()
            );
            println!("cargo:rustc-link-lib=static={libname}");
        }
    }

    // Platform-specific libraries
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=iconv");
    }

    // shell.o contains a main() function that conflicts with Rust's main (and
    // libfuzzer's main when fuzzing). We tell the linker to allow multiple
    // definitions and use the first one (Rust's/libfuzzer's).
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
    }

    // On macOS, use -Wl,-multiply_defined,suppress for the same effect
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-multiply_defined,suppress");
    }
}

fn configure_bash(bash_src: &PathBuf) {
    eprintln!("Configuring bash...");

    let status = Command::new("./configure")
        .current_dir(bash_src)
        .args([
            "--disable-nls",
            "--without-bash-malloc",
            "--disable-bang-history",
            "--disable-progcomp",
            "--disable-net-redirections",
        ])
        .status()
        .expect("Failed to run configure");

    assert!(status.success(), "Failed to configure bash");
}

fn build_bash(bash_src: &PathBuf) {
    eprintln!("Building bash...");

    let status = Command::new("make")
        .current_dir(bash_src)
        .args(["-j4"])
        .status()
        .expect("Failed to run make");

    assert!(status.success(), "Failed to build bash");
}

fn verify_subst_flags(bash_src: &Path) {
    // Verify that our hardcoded SX_* flag values match bash's actual values.
    // This prevents silent breakage if bash changes these values.
    let subst_h = bash_src.join("subst.h");
    let content = std::fs::read_to_string(&subst_h)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", subst_h.display(), e));

    // Expected values (must match safe_parse.c)
    let expected = [("SX_NOLONGJMP", "0x0040"), ("SX_NOERROR", "0x1000")];

    for (name, value) in expected {
        let pattern = format!("#define {name}	{value}");
        let alt_pattern = format!("#define {name} {value}");
        assert!(
            content.contains(&pattern) || content.contains(&alt_pattern),
            "Flag value mismatch: {name} is not {value} in bash/subst.h. \
             Update the values in safe_parse.c to match bash's subst.h"
        );
    }
    eprintln!("Verified SX_NOLONGJMP and SX_NOERROR flags match bash/subst.h");
}

fn compile_safe_parse(manifest_dir: &Path, bash_src: &Path, out_dir: &Path) {
    // Verify flag values before compiling
    verify_subst_flags(bash_src);

    eprintln!("Compiling safe_parse.c...");

    let safe_parse_c = manifest_dir.join("safe_parse.c");

    // Use the cc crate for better cross-platform support
    cc::Build::new()
        .file(&safe_parse_c)
        .define("HAVE_CONFIG_H", None)
        .define("SHELL", None)
        .include(bash_src)
        .include(bash_src.join("include"))
        .include(bash_src.join("lib"))
        .opt_level(2)
        .warnings(false) // Bash headers generate many warnings
        .out_dir(out_dir)
        .compile("safe_parse");

    eprintln!("Compiled safe_parse using cc crate");
}

fn create_static_library(bash_src: &Path, out_dir: &Path) {
    eprintln!("Creating static library from bash objects...");

    let lib_path = out_dir.join("libbash_parser.a");

    // Remove existing library
    let _ = std::fs::remove_file(&lib_path);

    // Core parser objects
    let core_objects = [
        "y.tab.o",
        "make_cmd.o",
        "dispose_cmd.o",
        "copy_cmd.o",
        "print_cmd.o",
        "general.o",
        "hashlib.o",
        "stringlib.o",
        "list.o",
        "xmalloc.o",
        "error.o",
        "subst.o",
        "syntax.o",
        "variables.o",
        "input.o",
        "flags.o",
        "shell.o",
        "eval.o",
        "execute_cmd.o",
        "expr.o",
        "trap.o",
        "unwind_prot.o",
        "pathexp.o",
        "sig.o",
        "test.o",
        "version.o",
        "alias.o",
        "array.o",
        "array2.o",
        "arrayfunc.o",
        "assoc.o",
        "braces.o",
        "bracecomp.o",
        "bashhist.o",
        "bashline.o",
        "locale.o",
        "findcmd.o",
        "redir.o",
        "pcomplete.o",
        "pcomplib.o",
        "hashcmd.o",
        "mailcheck.o",
        "jobs.o",
    ];

    // Collect existing object files
    let mut existing_objects: Vec<PathBuf> = Vec::new();
    for obj in &core_objects {
        let obj_path = bash_src.join(obj);
        if obj_path.exists() {
            existing_objects.push(obj_path);
        } else {
            eprintln!("Warning: Object file not found: {obj}");
        }
    }

    // Include .o files from support directories to resolve circular dependencies
    let support_dirs = [
        bash_src.join("lib").join("sh"),
        bash_src.join("lib").join("glob"),
        bash_src.join("lib").join("tilde"),
        bash_src.join("lib").join("readline"),
        bash_src.join("builtins"),
    ];
    for dir in &support_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "o") {
                    existing_objects.push(path);
                }
            }
        }
    }

    assert!(
        !existing_objects.is_empty(),
        "No object files found! Make sure bash is built."
    );

    // Note: safe_parse is compiled separately by the cc crate and linked
    // automatically via cargo:rustc-link-lib

    // Create static library using ar
    let mut ar_cmd = Command::new("ar");
    ar_cmd.arg("rcs").arg(&lib_path);

    for obj in &existing_objects {
        ar_cmd.arg(obj);
    }

    let status = ar_cmd.status().expect("Failed to run ar");

    assert!(status.success(), "Failed to create static library");

    // Run ranlib
    let status = Command::new("ranlib")
        .arg(&lib_path)
        .status()
        .expect("Failed to run ranlib");

    assert!(status.success(), "Failed to run ranlib");

    eprintln!("Created static library at {}", lib_path.display());
}

fn generate_bindings(bash_src: &Path, out_dir: &Path) {
    eprintln!("Generating FFI bindings...");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let bindings = bindgen::Builder::default()
        .header(manifest_dir.join("wrapper.h").to_str().unwrap())
        // Include paths for bash headers
        .clang_arg(format!("-I{}", bash_src.display()))
        .clang_arg(format!("-I{}", bash_src.join("include").display()))
        .clang_arg(format!("-I{}", bash_src.join("lib").display()))
        // Define macros that bash uses
        .clang_arg("-DHAVE_CONFIG_H")
        .clang_arg("-DSHELL")
        // Allowlist the types and functions we need
        .allowlist_function("parse_string_to_command")
        .allowlist_function("safe_parse_string_to_command")
        .allowlist_function("safe_parse_verbose")
        .allowlist_function("safe_parse_script")
        .allowlist_function("dispose_command")
        .allowlist_function("reset_parser")
        .allowlist_function("yyparse")
        // Initialization functions
        .allowlist_function("initialize_shell_builtins")
        .allowlist_function("initialize_traps")
        .allowlist_function("initialize_signals")
        .allowlist_function("initialize_shell_variables")
        .allowlist_function("initialize_job_control")
        .allowlist_function("initialize_bash_input")
        .allowlist_function("initialize_flags")
        // Command types
        .allowlist_type("COMMAND")
        .allowlist_type("command_type")
        .allowlist_type("WORD_DESC")
        .allowlist_type("WORD_LIST")
        .allowlist_type("REDIRECT")
        .allowlist_type("REDIRECTEE")
        .allowlist_type("r_instruction")
        // Command structures
        .allowlist_type("SIMPLE_COM")
        .allowlist_type("FOR_COM")
        .allowlist_type("ARITH_FOR_COM")
        .allowlist_type("SELECT_COM")
        .allowlist_type("CASE_COM")
        .allowlist_type("PATTERN_LIST")
        .allowlist_type("IF_COM")
        .allowlist_type("WHILE_COM")
        .allowlist_type("ARITH_COM")
        .allowlist_type("COND_COM")
        .allowlist_type("CONNECTION")
        .allowlist_type("FUNCTION_DEF")
        .allowlist_type("GROUP_COM")
        .allowlist_type("SUBSHELL_COM")
        .allowlist_type("COPROC_COM")
        // Global variables we need to access
        .allowlist_var("interactive")
        .allowlist_var("login_shell")
        .allowlist_var("posixly_correct")
        .allowlist_var("interactive_shell")
        .allowlist_var("startup_state")
        .allowlist_var("shell_initialized")
        .allowlist_var("parsing_command")
        // Enums and constants
        .allowlist_var("cm_.*")
        .allowlist_var("r_.*")
        .allowlist_var("COND_.*")
        .allowlist_var("W_.*")
        .allowlist_var("CMD_.*")
        .allowlist_var("CASEPAT_.*")
        // Rust-specific options
        .derive_debug(true)
        .derive_default(true)
        .generate_comments(true)
        .layout_tests(false)
        .generate()
        .expect("Failed to generate bindings");

    let bindings_path = out_dir.join("bindings.rs");
    bindings
        .write_to_file(&bindings_path)
        .expect("Failed to write bindings");

    eprintln!("Generated bindings at {}", bindings_path.display());
}
