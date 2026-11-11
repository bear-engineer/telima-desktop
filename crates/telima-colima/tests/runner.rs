use telima_colima::cli::{CommandRunner, RunError, TokioRunner};

fn sh(script: &str) -> Vec<String> {
    ["/bin/sh", "-c", script]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[tokio::test]
async fn streams_stderr_lines_and_returns_stdout_and_code() {
    let mut lines: Vec<String> = vec![];
    let out = TokioRunner
        .run(
            &sh("echo out; echo e1 >&2; echo e2 >&2; exit 3"),
            &mut |l| lines.push(l.to_owned()),
        )
        .await
        .unwrap();
    assert_eq!(out.code, Some(3));
    assert_eq!(out.stdout, "out\n");
    assert_eq!(lines, ["e1", "e2"]);
}

#[tokio::test]
async fn missing_program_is_not_found() {
    let argv = vec!["/nonexistent/telima-nope".to_string()];
    let err = TokioRunner.run(&argv, &mut |_| {}).await.unwrap_err();
    assert!(matches!(err, RunError::NotFound(_)), "{err:?}");
}
