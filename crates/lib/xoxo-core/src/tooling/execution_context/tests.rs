use std::collections::HashMap;

use super::*;

#[tokio::test]
async fn env_var_persists_between_commands() {
    let mut session = BashSession::spawn(BashOptions::default()).await.unwrap();
    session.run_command("export FOO=hello", 5).await.unwrap();
    let out = session.run_command("echo $FOO", 5).await.unwrap();
    assert_eq!(out.stdout.trim(), "hello");
    assert_eq!(out.exit_code, 0);
    session.kill().await;
}

#[tokio::test]
async fn working_directory_persists_between_commands() {
    let mut session = BashSession::spawn(BashOptions::default()).await.unwrap();
    session.run_command("cd /tmp", 5).await.unwrap();
    let out = session.run_command("pwd", 5).await.unwrap();
    assert_eq!(out.stdout.trim(), "/tmp");
    session.kill().await;
}

#[tokio::test]
async fn nonzero_exit_code_is_captured() {
    let mut session = BashSession::spawn(BashOptions::default()).await.unwrap();
    let out = session.run_command("(exit 42)", 5).await.unwrap();
    assert_eq!(out.exit_code, 42);
    assert!(!out.timed_out);
    session.kill().await;
}

#[tokio::test]
async fn stderr_is_captured_separately() {
    let mut session = BashSession::spawn(BashOptions::default()).await.unwrap();
    let out = session.run_command("echo error-msg >&2", 5).await.unwrap();
    assert!(out.stdout.is_empty());
    assert_eq!(out.stderr.trim(), "error-msg");
    session.kill().await;
}

#[tokio::test]
async fn timeout_is_enforced() {
    let mut session = BashSession::spawn(BashOptions::default()).await.unwrap();
    let out = session.run_command("sleep 10", 1).await.unwrap();
    assert!(out.timed_out);
    assert_eq!(out.exit_code, -1);
    session.kill().await;
}

#[tokio::test]
async fn default_session_inherits_path() {
    // The default BashOptions (inherit_env: true) must expose the parent's PATH
    // so that tools installed in non-standard locations (e.g. /opt/homebrew/bin)
    // are found without any manual configuration.
    let mut session = BashSession::spawn(BashOptions::default()).await.unwrap();
    let out = session.run_command("echo $PATH", 5).await.unwrap();
    assert!(!out.stdout.trim().is_empty(), "PATH should not be empty in an inherited-env session");
    session.kill().await;
}

#[tokio::test]
async fn isolated_session_does_not_inherit_parent_env() {
    // Inject a sentinel into the current process env. With inherit_env: false
    // the child bash session must NOT see this variable — env isolation is working.
    // SAFETY: single-threaded test context, no other code reads this var.
    unsafe { std::env::set_var("ISOLATION_SENTINEL", "must-not-leak") };
    let mut session = BashSession::spawn(BashOptions {
        inherit_env: false,
        login: false, // no user init files in an isolated session
        env_vars: HashMap::new(),
        ..BashOptions::default()
    })
    .await
    .unwrap();
    let out = session
        .run_command("echo ${ISOLATION_SENTINEL:-absent}", 5)
        .await
        .unwrap();
    assert_eq!(
        out.stdout.trim(),
        "absent",
        "isolated session must not inherit parent env vars"
    );
    session.kill().await;
}

#[tokio::test]
async fn env_vars_override_inherited_env() {
    // env_vars take precedence over the inherited environment.
    let mut env_vars = HashMap::new();
    env_vars.insert("CUSTOM_VAR".to_string(), "clawrster-test".to_string());
    let mut session = BashSession::spawn(BashOptions {
        inherit_env: true,
        env_vars,
        ..BashOptions::default()
    })
    .await
    .unwrap();
    let out = session.run_command("echo $CUSTOM_VAR", 5).await.unwrap();
    assert_eq!(out.stdout.trim(), "clawrster-test");
    session.kill().await;
}
