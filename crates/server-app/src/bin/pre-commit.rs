use std::env;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    let staged_files = if args.len() > 1 { &args[1..] } else { &[] };

    // Check if any Rust files are staged
    let has_rust_files = staged_files.iter().any(|file| {
        std::path::Path::new(file)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
    });
    if !has_rust_files {
        println!("No Rust files to check.");
        return;
    }

    // Run cargo fmt
    println!("Checking code formatting...");
    let fmt_status = Command::new("cargo")
        .args(["fmt", "--all", "--", "--check"])
        .status()
        .expect("Failed to run cargo fmt");

    if !fmt_status.success() {
        eprintln!("Formatting check failed. Please run 'cargo fmt --all' before committing.");
        std::process::exit(1);
    }
    println!("Formatting check passed.");

    // Run cargo clippy
    println!("Running clippy...");
    let clippy_status = Command::new("cargo")
        .args([
            "clippy",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ])
        .status()
        .expect("Failed to run cargo clippy");

    if !clippy_status.success() {
        eprintln!("Clippy check failed. Please fix the issues before committing.");
        std::process::exit(1);
    }
    println!("Clippy check passed.");

    // Run cargo check
    println!("Checking compilation...");
    let check_status = Command::new("cargo")
        .args(["check", "--all-targets", "--all-features"])
        .status()
        .expect("Failed to run cargo check");

    if !check_status.success() {
        eprintln!("Compilation check failed. Please fix the issues before committing.");
        std::process::exit(1);
    }
    println!("Compilation check passed.");

    // Run unit tests
    println!("Running unit tests...");
    let unit_test_status = Command::new("cargo")
        .args(["test", "--lib"])
        .status()
        .expect("Failed to run unit tests");

    if !unit_test_status.success() {
        eprintln!("Unit tests failed. Please fix the issues before committing.");
        std::process::exit(1);
    }
    println!("Unit tests passed.");

    // Run integration tests
    println!("Running integration tests...");
    let integration_test_status = Command::new("cargo")
        .args(["test", "integration::"])
        .status()
        .expect("Failed to run integration tests");

    if !integration_test_status.success() {
        eprintln!("Integration tests failed. Please fix the issues before committing.");
        std::process::exit(1);
    }
    println!("Integration tests passed.");

    println!("All checks passed! Commit is ready to be created.");
}
