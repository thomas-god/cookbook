use std::env;
use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;

use anyhow::{Context, bail};
use cookbook_domain::user::models::{UserId, UserRole};
use cookbook_infra::auth::{
    Argon2PasswordHasher, AuthServiceImpl, RandomSessionTokenGenerator, SqliteSessionRepository,
    SqliteUserRepository, SystemClock,
    models::{AuthUser, SessionPolicy, UserEmail},
    ports::{UserMaintenanceService, UserRepository},
};
use cookbook_infra::storage::SqliteDatabase;

const DATA_DIR_ENV: &str = "COOKBOOK_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";
const DATABASE_FILE_NAME: &str = "cookbook.sqlite3";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Some(options) = Options::from_env_and_args()? else {
        return Ok(());
    };

    run(options).await
}

async fn run(options: Options) -> anyhow::Result<()> {
    let database_path = options.data_dir.join(DATABASE_FILE_NAME);

    if !database_path.is_file() {
        bail!(
            "database not found at {}; check `--data-dir` or {DATA_DIR_ENV}",
            database_path.display()
        );
    }

    let database = SqliteDatabase::open(&database_path)
        .await
        .with_context(|| format!("failed to open {}", database_path.display()))?;
    let users = SqliteUserRepository::new(database.pool().clone());

    let user = match &options.command {
        Command::SetPassword { selector, .. }
        | Command::Promote { selector }
        | Command::Demote { selector } => selector.resolve(&users).await?,
    };

    let auth = AuthServiceImpl::new(
        users,
        Argon2PasswordHasher::new(),
        SqliteSessionRepository::new(database.pool().clone()),
        SystemClock::new(),
        RandomSessionTokenGenerator::new(),
        SessionPolicy::default(),
    );

    match options.command {
        Command::SetPassword { password_stdin, .. } => {
            let password = read_password(password_stdin)?;
            auth.reset_password(user.id(), password).await?;

            println!(
                "password updated for {} ({})",
                user.display_name(),
                user.id().as_str()
            );
        }
        Command::Promote { .. } => {
            let updated = auth.set_role(user.id(), UserRole::Admin).await?;

            println!(
                "user {} ({}) is now {}",
                updated.display_name(),
                updated.id().as_str(),
                updated.role().as_str()
            );
        }
        Command::Demote { .. } => {
            let updated = auth.set_role(user.id(), UserRole::User).await?;

            println!(
                "user {} ({}) is now {}",
                updated.display_name(),
                updated.id().as_str(),
                updated.role().as_str()
            );
        }
    }

    Ok(())
}

fn read_password(from_stdin: bool) -> anyhow::Result<String> {
    if from_stdin {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("failed to read password from stdin")?;

        let password = line.trim_end_matches(['\r', '\n']).to_string();

        if password.is_empty() {
            bail!("password read from stdin is empty");
        }

        return Ok(password);
    }

    if !std::io::stdin().is_terminal() {
        bail!(
            "no interactive terminal available; use `--password-stdin` to read the password from standard input"
        );
    }

    let password =
        rpassword::prompt_password("New password: ").context("failed to read password")?;

    if password.is_empty() {
        bail!("password must not be empty");
    }

    let confirmation = rpassword::prompt_password("Confirm password: ")
        .context("failed to read password confirmation")?;

    if password != confirmation {
        bail!("passwords do not match");
    }

    Ok(password)
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    data_dir: PathBuf,
    command: Command,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    SetPassword {
        selector: Selector,
        password_stdin: bool,
    },
    Promote {
        selector: Selector,
    },
    Demote {
        selector: Selector,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum Selector {
    Id(UserId),
    Email(UserEmail),
}

impl Selector {
    fn from_flags(id: Option<String>, email: Option<String>) -> anyhow::Result<Self> {
        match (id, email) {
            (Some(_), Some(_)) => bail!("provide exactly one of `--id` or `--email`"),
            (None, None) => bail!("provide `--id` or `--email`"),
            (Some(id), None) => {
                let id = id.trim();

                if id.is_empty() {
                    bail!("`--id` must not be empty");
                }

                Ok(Self::Id(UserId::new(id.to_string())))
            }
            (None, Some(email)) => {
                let email = UserEmail::try_new(email).context("invalid email address")?;

                Ok(Self::Email(email))
            }
        }
    }

    async fn resolve(&self, users: &SqliteUserRepository) -> anyhow::Result<AuthUser> {
        let user = match self {
            Self::Id(id) => users.find_by_id(id).await?,
            Self::Email(email) => users.find_by_email(email).await?,
        };

        user.with_context(|| format!("user not found: {}", self.describe()))
    }

    fn describe(&self) -> String {
        match self {
            Self::Id(id) => id.as_str().to_string(),
            Self::Email(email) => email.as_str().to_string(),
        }
    }
}

impl Options {
    /// Returns `None` when `--help` was requested.
    fn from_env_and_args() -> anyhow::Result<Option<Self>> {
        let data_dir = env::var(DATA_DIR_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));

        Self::parse(data_dir, env::args().skip(1))
    }

    fn parse(
        mut data_dir: PathBuf,
        arguments: impl IntoIterator<Item = String>,
    ) -> anyhow::Result<Option<Self>> {
        let mut arguments = arguments.into_iter();
        let mut subcommand = None;
        let mut id = None;
        let mut email = None;
        let mut password_stdin = false;

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-h" | "--help" => {
                    print_help();
                    return Ok(None);
                }
                "--data-dir" => {
                    data_dir =
                        PathBuf::from(arguments.next().context("`--data-dir` requires a path")?);
                }
                "--id" => {
                    let value = arguments.next().context("`--id` requires a value")?;

                    if id.replace(value).is_some() {
                        bail!("`--id` was provided more than once");
                    }
                }
                "--email" => {
                    let value = arguments.next().context("`--email` requires a value")?;

                    if email.replace(value).is_some() {
                        bail!("`--email` was provided more than once");
                    }
                }
                "--password-stdin" => {
                    if password_stdin {
                        bail!("`--password-stdin` was provided more than once");
                    }

                    password_stdin = true;
                }
                other if other.starts_with('-') => bail!("unknown argument `{other}`"),
                other => {
                    if subcommand.replace(other.to_string()).is_some() {
                        bail!("unexpected argument `{other}`");
                    }
                }
            }
        }

        let Some(subcommand) = subcommand else {
            bail!("missing command; expected `set-password`, `promote`, or `demote`");
        };

        let selector = Selector::from_flags(id, email)?;

        let command = match subcommand.as_str() {
            "set-password" => Command::SetPassword {
                selector,
                password_stdin,
            },
            "promote" => {
                reject_password_stdin(password_stdin, "promote")?;
                Command::Promote { selector }
            }
            "demote" => {
                reject_password_stdin(password_stdin, "demote")?;
                Command::Demote { selector }
            }
            other => bail!("unknown command `{other}`"),
        };

        Ok(Some(Self { data_dir, command }))
    }
}

fn reject_password_stdin(value: bool, command: &str) -> anyhow::Result<()> {
    if value {
        bail!("`--password-stdin` is only valid with `set-password`, not `{command}`");
    }

    Ok(())
}

fn print_help() {
    println!("Usage:");
    println!(
        "  cookbook-admin [--data-dir <PATH>] set-password (--id <ID> | --email <EMAIL>) [--password-stdin]"
    );
    println!("  cookbook-admin [--data-dir <PATH>] promote (--id <ID> | --email <EMAIL>)");
    println!("  cookbook-admin [--data-dir <PATH>] demote (--id <ID> | --email <EMAIL>)");
    println!();
    println!("Maintains existing users directly in the SQLite database.");
    println!("This is an emergency operator tool: it requires shell access to the");
    println!("application container or its data volume and is not exposed over the network.");
    println!();
    println!("Commands:");
    println!("  set-password  Reset a user's password without the current password");
    println!("  promote       Change a user's role to admin");
    println!("  demote        Change an admin's role to user (the last admin is protected)");
    println!();
    println!("Options:");
    println!("  --id <ID>          Select the user by id");
    println!("  --email <EMAIL>    Select the user by email");
    println!("  --password-stdin   Read the new password from one stdin line");
    println!("                     (set-password only)");
    println!("  --data-dir <PATH>  Data directory containing {DATABASE_FILE_NAME}");
    println!("                     [env: {DATA_DIR_ENV}] [default: ./{DEFAULT_DATA_DIR}]");
    println!("  -h, --help         Print help");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> anyhow::Result<Option<Options>> {
        Options::parse(
            PathBuf::from("data"),
            arguments.iter().map(|argument| (*argument).to_string()),
        )
    }

    #[test]
    fn parses_set_password_by_email_with_stdin() {
        let options = parse(&[
            "set-password",
            "--email",
            " Alice@Example.com ",
            "--password-stdin",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(options.data_dir, PathBuf::from("data"));
        assert_eq!(
            options.command,
            Command::SetPassword {
                selector: Selector::Email(UserEmail::try_new("alice@example.com").unwrap()),
                password_stdin: true,
            }
        );
    }

    #[test]
    fn parses_promote_by_id_and_a_data_directory() {
        let options = parse(&["promote", "--data-dir", "/srv/cookbook", "--id", "user-1"])
            .unwrap()
            .unwrap();

        assert_eq!(options.data_dir, PathBuf::from("/srv/cookbook"));
        assert_eq!(
            options.command,
            Command::Promote {
                selector: Selector::Id(UserId::new("user-1".to_string())),
            }
        );
    }

    #[test]
    fn parses_demote_by_email() {
        let options = parse(&["demote", "--email", "admin@example.com"])
            .unwrap()
            .unwrap();

        assert_eq!(
            options.command,
            Command::Demote {
                selector: Selector::Email(UserEmail::try_new("admin@example.com").unwrap()),
            }
        );
    }

    #[test]
    fn requires_exactly_one_selector() {
        assert!(parse(&["promote"]).is_err());
        assert!(parse(&["promote", "--id", "user-1", "--email", "a@example.com"]).is_err());
    }

    #[test]
    fn rejects_invalid_email_selectors() {
        assert!(parse(&["promote", "--email", "not-an-email"]).is_err());
    }

    #[test]
    fn rejects_password_stdin_for_role_commands() {
        assert!(parse(&["promote", "--id", "user-1", "--password-stdin"]).is_err());
        assert!(parse(&["demote", "--id", "user-1", "--password-stdin"]).is_err());
    }

    #[test]
    fn rejects_unknown_arguments_and_commands() {
        assert!(parse(&["set-password", "--id", "user-1", "--force"]).is_err());
        assert!(parse(&["list", "--id", "user-1"]).is_err());
        assert!(parse(&["promote", "--id", "user-1", "extra"]).is_err());
    }

    #[test]
    fn help_returns_none() {
        assert!(parse(&["--help"]).unwrap().is_none());
    }
}
