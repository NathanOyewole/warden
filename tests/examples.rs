//! Integration tests over examples/accept/*.wd and examples/reject/*.wd.
//!
//! These aren't unit tests of individual checker rules (checker.rs
//! already has those) -- they're the credibility layer for the whole
//! project: real, readable Warden programs, each demonstrating exactly
//! one rule, verified to actually behave the way the article claims.
//!
//! We deliberately use `check_source` (checker only, never executes)
//! rather than `run_source` here. A couple of the accept examples use
//! `while` loops whose condition never becomes false -- Warden has no
//! reassignment statement, so nothing in the language can flip a loop
//! condition from truthy to falsy. Running them would hang forever.
//! Proving the checker accepts them doesn't require running them.

use std::fs;
use std::path::Path;

fn read_examples(dir: &str) -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join(dir);
    let mut entries: Vec<(String, String)> = fs::read_dir(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {}", path.display(), e))
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            let source = fs::read_to_string(entry.path()).unwrap();
            (name, source)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn all_accept_examples_pass_the_checker() {
    for (name, source) in read_examples("accept") {
        let result = warden::check_source(&source);
        assert!(
            result.is_ok(),
            "expected {} to be ACCEPTED by the checker, but it was rejected: {:?}",
            name,
            result
        );
    }
}

#[test]
fn all_reject_examples_fail_the_checker() {
    for (name, source) in read_examples("reject") {
        let result = warden::check_source(&source);
        assert!(
            result.is_err(),
            "expected {} to be REJECTED by the checker, but it was accepted",
            name
        );
    }
}

#[test]
fn accept_examples_that_dont_loop_forever_also_run_correctly() {
    // Any accept/ example without `while` can safely go through the
    // full pipeline, proving the interpreter agrees with the checker.
    for (name, source) in read_examples("accept") {
        if source.contains("while") {
            continue;
        }
        let result = warden::run_source(&source);
        assert!(result.is_ok(), "expected {} to run successfully: {:?}", name, result);
    }
}
