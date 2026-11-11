use std::collections::HashMap;
use std::path::{Path, PathBuf};
use telima_colima::locate::locate_colima;

fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k: &str| map.get(k).cloned()
}

fn files(paths: &[&str]) -> impl Fn(&Path) -> bool {
    let set: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    move |p: &Path| set.iter().any(|f| f == p)
}

#[test]
fn finds_in_path_entry() {
    let got = locate_colima(env(&[("PATH", "/a:/b")]), files(&["/b/colima"]));
    assert_eq!(got, Some(PathBuf::from("/b/colima")));
}

#[test]
fn path_wins_over_fixed_locations() {
    let got = locate_colima(
        env(&[("PATH", "/x")]),
        files(&["/opt/homebrew/bin/colima", "/x/colima"]),
    );
    assert_eq!(got, Some(PathBuf::from("/x/colima")));
}

#[test]
fn fixed_order_homebrew_before_usr_local() {
    let got = locate_colima(
        env(&[("PATH", "/usr/bin:/bin")]),
        files(&["/usr/local/bin/colima", "/opt/homebrew/bin/colima"]),
    );
    assert_eq!(got, Some(PathBuf::from("/opt/homebrew/bin/colima")));
}

#[test]
fn finds_user_local_bin_via_home() {
    let got = locate_colima(
        env(&[("HOME", "/Users/tester")]),
        files(&["/Users/tester/.local/bin/colima"]),
    );
    assert_eq!(got, Some(PathBuf::from("/Users/tester/.local/bin/colima")));
}

#[test]
fn finds_linuxbrew() {
    let got = locate_colima(
        env(&[("PATH", "")]),
        files(&["/home/linuxbrew/.linuxbrew/bin/colima"]),
    );
    assert_eq!(
        got,
        Some(PathBuf::from("/home/linuxbrew/.linuxbrew/bin/colima"))
    );
}

#[test]
fn empty_path_entries_are_skipped() {
    let got = locate_colima(env(&[("PATH", "::/a")]), files(&["/a/colima"]));
    assert_eq!(got, Some(PathBuf::from("/a/colima")));
}

#[test]
fn none_when_absent() {
    assert_eq!(locate_colima(env(&[("PATH", "/a")]), files(&[])), None);
}
