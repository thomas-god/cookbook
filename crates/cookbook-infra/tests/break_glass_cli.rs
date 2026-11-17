use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use cookbook_config::defaults;
use cookbook_domain::user::models::{UserId, UserRole, Username};
use cookbook_infra::auth::models::{AuthUser, UserEmail};
use cookbook_infra::auth::ports::{PasswordHasher, UserRepository};
use cookbook_infra::auth::{Argon2PasswordHasher, SqliteUserRepository};
use cookbook_infra::storage::SqliteDatabase;
use tempfile::tempdir;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_cookbook-admin")
}

async fn seed_user(directory: &Path, id: &str, email: &str, password: &str, role: UserRole) {
    let database = SqliteDatabase::open(directory.join(defaults::DATABASE_FILE_NAME))
        .await
        .unwrap();
    let users = SqliteUserRepository::new(database.pool().clone());
    let password_hash = Argon2PasswordHasher::new()
        .hash_password(password)
        .await
        .unwrap();

    users
        .save_user(AuthUser::new(
            UserId::new(id.to_string()),
            UserEmail::try_new(email).unwrap(),
            Some(Username::new("Test User".to_string())),
            password_hash,
            role,
        ))
        .await
        .unwrap();
}

async fn load_user(directory: &Path, email: &str) -> AuthUser {
    let database = SqliteDatabase::open(directory.join(defaults::DATABASE_FILE_NAME))
        .await
        .unwrap();
    let users = SqliteUserRepository::new(database.pool().clone());

    users
        .find_by_email(&UserEmail::try_new(email).unwrap())
        .await
        .unwrap()
        .expect("user should exist")
}

fn run(directory: &Path, arguments: &[&str], stdin: Option<&str>) -> Output {
    let mut command = Command::new(binary());
    command
        .arg("--data-dir")
        .arg(directory)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });

    let mut child = command.spawn().unwrap();

    if let Some(input) = stdin {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }

    child.wait_with_output().unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn resets_a_password_by_email() {
    let directory = tempdir().unwrap();
    seed_user(
        directory.path(),
        "user-1",
        "alice@example.com",
        "old-password",
        UserRole::Admin,
    )
    .await;

    let output = run(
        directory.path(),
        &[
            "set-password",
            "--email",
            " ALICE@Example.com ",
            "--password-stdin",
        ],
        Some("new-password\n"),
    );

    assert_success(&output);

    let user = load_user(directory.path(), "alice@example.com").await;
    let hasher = Argon2PasswordHasher::new();

    assert!(
        hasher
            .verify_password("new-password", user.password_hash())
            .await
            .unwrap()
    );
    assert!(
        !hasher
            .verify_password("old-password", user.password_hash())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn resets_a_password_by_id_and_promotes_the_user() {
    let directory = tempdir().unwrap();
    seed_user(
        directory.path(),
        "user-1",
        "alice@example.com",
        "old-password",
        UserRole::User,
    )
    .await;

    let output = run(
        directory.path(),
        &["set-password", "--id", "user-1", "--password-stdin"],
        Some("new-password\n"),
    );
    assert_success(&output);

    let output = run(directory.path(), &["promote", "--id", "user-1"], None);
    assert_success(&output);

    let user = load_user(directory.path(), "alice@example.com").await;

    assert_eq!(user.role(), UserRole::Admin);
    assert!(
        Argon2PasswordHasher::new()
            .verify_password("new-password", user.password_hash())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn demotes_an_admin_when_another_admin_exists() {
    let directory = tempdir().unwrap();
    seed_user(
        directory.path(),
        "admin-1",
        "admin@example.com",
        "password",
        UserRole::Admin,
    )
    .await;
    seed_user(
        directory.path(),
        "admin-2",
        "other@example.com",
        "password",
        UserRole::Admin,
    )
    .await;

    let output = run(directory.path(), &["demote", "--id", "admin-1"], None);
    assert_success(&output);

    let user = load_user(directory.path(), "admin@example.com").await;

    assert_eq!(user.role(), UserRole::User);
}

#[tokio::test]
async fn refuses_to_demote_the_last_admin() {
    let directory = tempdir().unwrap();
    seed_user(
        directory.path(),
        "admin-1",
        "admin@example.com",
        "password",
        UserRole::Admin,
    )
    .await;

    let output = run(directory.path(), &["demote", "--id", "admin-1"], None);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("last administrator"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let user = load_user(directory.path(), "admin@example.com").await;

    assert_eq!(user.role(), UserRole::Admin);
}

#[tokio::test]
async fn rejects_a_missing_database_without_creating_it() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join(defaults::DATABASE_FILE_NAME);

    let output = run(directory.path(), &["promote", "--id", "user-1"], None);

    assert!(!output.status.success());
    assert!(!database_path.exists());
}
