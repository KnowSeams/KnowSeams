// CLI bug reproduction tests
// WHY: Document known CLI regressions as failing tests so they are not silently broken further.
// These tests FAIL on current code by design — they encode the expected correct behaviour.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn seams_bin() -> &'static str {
    env!("CARGO_BIN_EXE_seams")
}

// ── Bug 1: Arbitrary filename rejected ───────────────────────────────────────
//
// The CLI rejects any file whose name does not match the `*-0.txt` Gutenberg
// convention, making it impossible to segment an arbitrary text file without
// first renaming it.
//
// Root cause: process_single_file_mode checks `file_name.ends_with("-0.txt")`
// and bails with an error suggesting renaming. (src/main.rs ~L473)

/// Any readable .txt file should be segmentable, not just Gutenberg *-0.txt files.
#[test]
fn test_cli_accepts_arbitrary_txt_filename() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("my_novel.txt");
    fs::write(&file, "Hello world. This is a test. Another sentence here.").unwrap();

    let output = Command::new(seams_bin())
        .arg(&file)
        .current_dir(dir.path())
        .output()
        .expect("failed to run seams binary");

    // BUG: exits 1 with "File does not match expected pattern … must end in '-0.txt'"
    assert!(
        output.status.success(),
        "seams should accept any readable .txt file, not only files ending in '-0.txt'.\n\
         stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
}

/// Non-.txt extensions (e.g. plain file with no extension) should produce a clear
/// error, but a file named e.g. `chapter1.txt` must not be rejected solely because
/// of its stem.
#[test]
fn test_cli_accepts_hyphenated_name_without_zero() {
    let dir = TempDir::new().unwrap();
    // Has a hyphen but NOT the -0 suffix — currently rejected
    let file = dir.path().join("pg12345.txt");
    fs::write(&file, "Once upon a time. The end.").unwrap();

    let output = Command::new(seams_bin())
        .arg(&file)
        .current_dir(dir.path())
        .output()
        .expect("failed to run seams binary");

    // BUG: exits 1 because the stem is not `<something>-0`
    assert!(
        output.status.success(),
        "seams should process 'pg12345.txt' without requiring the '-0' suffix.\n\
         stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );
}

// ── Bug 2: Structured JSON tracing written to stdout instead of stderr ────────
//
// tracing_subscriber::fmt().json().init() defaults to stdout. Every info!()
// call emits a JSON line on stdout, making it impossible to pipe or redirect
// the actual output without first stripping the tracing lines.
//
// Root cause: tracing subscriber is not configured with .with_writer(stderr).
// (src/main.rs ~L781)

/// Running the binary in file mode must not emit JSON tracing lines on stdout.
/// Structured logs belong on stderr so that stdout is clean for downstream use.
#[test]
fn test_cli_file_mode_stdout_contains_no_json_tracing() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("book-0.txt");
    fs::write(&file, "Hello world. This is a test. Another sentence here.").unwrap();

    let output = Command::new(seams_bin())
        .arg(&file)
        .current_dir(dir.path())
        .output()
        .expect("failed to run seams binary");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // BUG: stdout currently contains lines like
    //   {"timestamp":"…","level":"INFO","fields":{"message":"Starting seams"}}
    assert!(
        !stdout.contains("{\"timestamp\""),
        "stdout must not contain JSON tracing output.\n\
         Structured logs should go to stderr, not stdout.\n\
         stdout was:\n{}",
        stdout,
    );
}

/// Running with --debug-text must emit only sentence lines on stdout.
/// JSON tracing lines must not be mixed with the TSV sentence output.
#[test]
fn test_cli_debug_text_stdout_contains_no_json_tracing() {
    let output = Command::new(seams_bin())
        .args(["--debug-text", "Hello world. This is a test."])
        .output()
        .expect("failed to run seams binary");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // BUG: stdout currently contains JSON tracing lines before and between
    // the real TSV sentence lines, e.g.:
    //   {"timestamp":"…","level":"INFO","fields":{"message":"Starting seams"}}
    //   0  Hello world.  (1,1,1,12)  …
    assert!(
        !stdout.contains("{\"timestamp\""),
        "--debug-text stdout must not contain JSON tracing lines.\n\
         Only TSV sentence output should appear on stdout.\n\
         stdout was:\n{}",
        stdout,
    );
}

/// JSON tracing lines that are currently on stdout should instead appear on stderr.
#[test]
fn test_cli_json_tracing_appears_on_stderr_not_stdout() {
    let output = Command::new(seams_bin())
        .args(["--debug-text", "Hello world. This is a test."])
        .output()
        .expect("failed to run seams binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // BUG: stderr is currently empty — the logs are on stdout instead.
    assert!(
        stderr.contains("Starting seams"),
        "Structured log messages should appear on stderr.\n\
         stderr was empty (logs are going to stdout instead).\n\
         stderr: {}\nstdout: {}",
        stderr,
        String::from_utf8_lossy(&output.stdout),
    );
}
