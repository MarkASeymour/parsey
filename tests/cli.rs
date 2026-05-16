use std::ffi::OsStr;
use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

fn run<I, S>(args: I, stdin: &[u8]) -> (i32, String, String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Command::cargo_bin("parsey")
        .unwrap()
        .env("NO_COLOR", "1")
        .args(args)
        .write_stdin(stdin.to_vec())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn match_in_stdin_exits_zero() {
    let (code, stdout, _) = run(["foo"], b"hello foo world\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("hello foo world"), "stdout: {stdout}");
}

#[test]
fn no_match_exits_one() {
    let (code, stdout, _) = run(["foo"], b"bar\n");
    assert_eq!(code, 1);
    assert!(stdout.is_empty(), "stdout: {stdout}");
}

#[test]
fn invalid_pattern_exits_two() {
    let (code, _, stderr) = run(["("], b"anything\n");
    assert_eq!(code, 2);
    assert!(stderr.contains("parsey:"), "stderr: {stderr}");
}

#[test]
fn missing_pattern_exits_two() {
    let (code, _, stderr) = run(Vec::<&str>::new(), b"");
    assert_eq!(code, 2);
    assert!(stderr.contains("Usage:"), "stderr: {stderr}");
}

#[test]
fn unknown_flag_exits_two() {
    let (code, _, stderr) = run(["-z", "foo"], b"");
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown flag"), "stderr: {stderr}");
}

#[test]
fn line_number_in_output() {
    let (code, stdout, _) = run(["bar"], b"foo\nbar\nbaz\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("2: bar"), "stdout: {stdout}");
}

#[test]
fn case_insensitive_flag() {
    let (code, stdout, _) = run(["-i", "foo"], b"FOO\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("FOO"), "stdout: {stdout}");
}

#[test]
fn bundled_flags_i_and_r() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("a.txt"), "FOO\n").unwrap();
    let (code, stdout, _) = run(["-iR", "foo", tmp.path().to_str().unwrap()], b"");
    assert_eq!(code, 0);
    assert!(stdout.contains("a.txt"), "stdout: {stdout}");
    assert!(stdout.contains("FOO"), "stdout: {stdout}");
}

#[test]
fn double_dash_terminator_lets_pattern_start_with_dash() {
    let (code, stdout, _) = run(["--", "-foo"], b"x-foo y\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("-foo"), "stdout: {stdout}");
}

#[test]
fn dash_means_stdin() {
    let (code, stdout, _) = run(["foo", "-"], b"foo bar\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("foo bar"), "stdout: {stdout}");
}

#[test]
fn multiple_files_show_filename() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("a.txt");
    let b = tmp.path().join("b.txt");
    fs::write(&a, "foo\n").unwrap();
    fs::write(&b, "foo\n").unwrap();
    let (code, stdout, _) = run(["foo", a.to_str().unwrap(), b.to_str().unwrap()], b"");
    assert_eq!(code, 0);
    assert!(stdout.contains("a.txt"), "stdout: {stdout}");
    assert!(stdout.contains("b.txt"), "stdout: {stdout}");
}

#[test]
fn single_file_omits_filename() {
    let tmp = TempDir::new().unwrap();
    let a = tmp.path().join("only.txt");
    fs::write(&a, "foo\n").unwrap();
    let (code, stdout, _) = run(["foo", a.to_str().unwrap()], b"");
    assert_eq!(code, 0);
    assert!(!stdout.contains("only.txt"), "stdout: {stdout}");
    assert!(stdout.contains("foo"), "stdout: {stdout}");
}

#[test]
fn directory_without_r_errors_to_stderr() {
    let tmp = TempDir::new().unwrap();
    let (code, _, stderr) = run(["foo", tmp.path().to_str().unwrap()], b"");
    assert_eq!(code, 1);
    assert!(stderr.contains("Is a directory"), "stderr: {stderr}");
}

#[test]
fn recursive_walks_nested_directories() {
    let tmp = TempDir::new().unwrap();
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("nested.txt"), "needle\n").unwrap();
    let (code, stdout, _) = run(["-r", "needle", tmp.path().to_str().unwrap()], b"");
    assert_eq!(code, 0);
    assert!(stdout.contains("nested.txt"), "stdout: {stdout}");
}

#[test]
fn r_and_capital_r_both_enable_recursion() {
    let tmp = TempDir::new().unwrap();
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("f.txt"), "hit\n").unwrap();

    for flag in ["-r", "-R"] {
        let (code, stdout, _) = run([flag, "hit", tmp.path().to_str().unwrap()], b"");
        assert_eq!(code, 0, "flag {flag}");
        assert!(stdout.contains("f.txt"), "flag {flag} stdout: {stdout}");
    }
}

#[test]
fn missing_file_reports_but_continues_others() {
    let tmp = TempDir::new().unwrap();
    let exists = tmp.path().join("exists.txt");
    fs::write(&exists, "hit\n").unwrap();
    let missing = tmp.path().join("does_not_exist.txt");
    let (code, stdout, stderr) = run(
        ["hit", missing.to_str().unwrap(), exists.to_str().unwrap()],
        b"",
    );
    assert_eq!(code, 0);
    assert!(stderr.contains("does_not_exist.txt"), "stderr: {stderr}");
    assert!(stdout.contains("exists.txt"), "stdout: {stdout}");
}

#[test]
fn crlf_line_endings_stripped_before_end_anchor() {
    let (code, stdout, _) = run(["foo$"], b"foo\r\nbar\r\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("1: foo"), "stdout: {stdout}");
}

#[test]
fn invalid_utf8_line_skipped_valid_line_kept() {
    let (code, stdout, _) = run(["foo"], b"\xff\xfe foo bytes\nfoo\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("2: foo"), "stdout: {stdout}");
    assert_eq!(stdout.matches("foo").count(), 1, "stdout: {stdout}");
}

#[test]
fn multiple_hits_in_single_line() {
    let (code, stdout, _) = run(["ab"], b"ababab\n");
    assert_eq!(code, 0);
    assert!(stdout.contains("ababab"), "stdout: {stdout}");
}

#[test]
fn recursive_default_input_is_cwd() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("x.txt"), "needle\n").unwrap();
    let out = Command::cargo_bin("parsey")
        .unwrap()
        .env("NO_COLOR", "1")
        .current_dir(tmp.path())
        .args(["-r", "needle"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("x.txt"), "stdout: {stdout}");
}
