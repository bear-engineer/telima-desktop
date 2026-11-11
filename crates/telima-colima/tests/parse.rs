use telima_colima::parse::{ParseError, parse_log_line, parse_ls, parse_version};
use telima_core::profile::{ProfileStatus, ProgressLevel, Runtime};

const LS_MIXED: &str = include_str!("fixtures/ls_mixed.jsonl");
const LS_EMPTY: &str = include_str!("fixtures/ls_empty.txt");
const LOG: &str = include_str!("fixtures/colima_start.log");
const VERSION: &str = include_str!("fixtures/colima_version.txt");

#[test]
fn ls_running_profile_maps_all_fields() {
    let profiles = parse_ls(LS_MIXED).unwrap();
    let d = &profiles[0];
    assert_eq!(d.name.as_str(), "default");
    assert_eq!(d.status, ProfileStatus::Running);
    assert_eq!(d.arch, "aarch64");
    assert_eq!(d.cpus, 8);
    assert_eq!(d.memory_bytes, 8_589_934_592);
    assert_eq!(d.disk_bytes, 107_374_182_400);
    assert_eq!(d.address.as_deref(), Some("192.168.64.2"));
    assert_eq!(d.runtime, Runtime::Docker);
}

#[test]
fn ls_status_and_missing_address_handled() {
    let profiles = parse_ls(LS_MIXED).unwrap();
    assert_eq!(profiles.len(), 4);
    assert_eq!(profiles[1].status, ProfileStatus::Stopped);
    assert_eq!(profiles[1].address, None, "missing address key -> None");
    assert_eq!(profiles[2].status, ProfileStatus::Other("Starting".into()));
    assert_eq!(profiles[2].address, None, "empty address -> None");
    assert_eq!(profiles[3].status, ProfileStatus::Other("Broken".into()));
}

#[test]
fn ls_empty_output_is_no_profiles() {
    assert!(parse_ls(LS_EMPTY).unwrap().is_empty());
    assert!(parse_ls("\n  \n").unwrap().is_empty());
}

#[test]
fn ls_invalid_json_reports_line() {
    let first = LS_MIXED.lines().next().unwrap();
    let err = parse_ls(&format!("{first}\nnot json\n")).unwrap_err();
    assert!(
        matches!(err, ParseError::Json { line: 2, .. }),
        "got {err:?}"
    );
}

#[test]
fn ls_unknown_runtime_is_error() {
    let line = r#"{"name":"x","status":"Running","arch":"aarch64","cpus":1,"memory":1,"disk":1,"runtime":"podman"}"#;
    assert_eq!(
        parse_ls(line).unwrap_err(),
        ParseError::Runtime("podman".into())
    );
}

#[test]
fn ls_invalid_name_is_error() {
    let line = r#"{"name":"Bad_Name","status":"Running","arch":"aarch64","cpus":1,"memory":1,"disk":1,"runtime":"docker"}"#;
    assert_eq!(
        parse_ls(line).unwrap_err(),
        ParseError::Name("Bad_Name".into())
    );
}

#[test]
fn log_info_line_extracts_msg() {
    let lines: Vec<&str> = LOG.lines().collect();
    let ev = parse_log_line(lines[0]);
    assert_eq!(ev.level, ProgressLevel::Info);
    assert_eq!(ev.message, "starting colima profile=telima-test-a");
}

#[test]
fn log_warn_keeps_escaped_quotes() {
    let lines: Vec<&str> = LOG.lines().collect();
    let ev = parse_log_line(lines[2]);
    assert_eq!(ev.level, ProgressLevel::Warn);
    assert_eq!(ev.message, r#"mount type "sshfs" is slower than virtiofs"#);
}

#[test]
fn log_error_level() {
    let lines: Vec<&str> = LOG.lines().collect();
    let ev = parse_log_line(lines[3]);
    assert_eq!(ev.level, ProgressLevel::Error);
    assert_eq!(ev.message, "failed to start: exit status 1");
}

#[test]
fn log_plain_line_is_info_with_whole_text() {
    let ev = parse_log_line("Plain progress line without a prefix");
    assert_eq!(ev.level, ProgressLevel::Info);
    assert_eq!(ev.message, "Plain progress line without a prefix");
}

#[test]
fn version_first_line_parses() {
    assert_eq!(parse_version(VERSION).unwrap(), (0, 10, 3));
    assert_eq!(parse_version("colima version 1.2.30").unwrap(), (1, 2, 30));
}

#[test]
fn version_garbage_is_error() {
    assert!(matches!(
        parse_version("hello\n"),
        Err(ParseError::Version(_))
    ));
    assert!(matches!(parse_version(""), Err(ParseError::Version(_))));
}
