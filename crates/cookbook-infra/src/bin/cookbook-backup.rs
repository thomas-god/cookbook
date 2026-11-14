use std::env;
use std::path::PathBuf;

use anyhow::{Context, bail};
use cookbook_infra::storage::backup_database;
use tracing_subscriber::EnvFilter;

const DATA_DIR_ENV: &str = "COOKBOOK_DATA_DIR";
const OUTPUT_DIR_ENV: &str = "COOKBOOK_BACKUP_DIR";
const KEEP_ENV: &str = "COOKBOOK_BACKUP_KEEP";
const DEFAULT_DATA_DIR: &str = "data";
const DEFAULT_KEEP: usize = 14;
const DATABASE_FILE_NAME: &str = "cookbook.sqlite3";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let options = match Options::from_env_and_args()? {
        Some(options) => options,
        None => return Ok(()),
    };

    let source = options.data_dir.join(DATABASE_FILE_NAME);
    let outcome = backup_database(&source, &options.output_dir, options.keep).await?;

    println!("backup written to {}", outcome.path.display());

    if outcome.removed > 0 {
        println!("removed {} old backup(s)", outcome.removed);
    }

    Ok(())
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[derive(Debug)]
struct Options {
    data_dir: PathBuf,
    output_dir: PathBuf,
    keep: usize,
}

impl Options {
    /// Returns `None` when `--help` was requested.
    fn from_env_and_args() -> anyhow::Result<Option<Self>> {
        let mut data_dir = optional_path(DATA_DIR_ENV).unwrap_or_else(default_data_dir);
        let mut output_dir = optional_path(OUTPUT_DIR_ENV);
        let mut keep = optional_keep()?.unwrap_or(DEFAULT_KEEP);

        let mut arguments = env::args().skip(1);

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--data-dir" => {
                    data_dir =
                        PathBuf::from(arguments.next().context("`--data-dir` requires a path")?);
                }
                "--output-dir" => {
                    output_dir = Some(PathBuf::from(
                        arguments.next().context("`--output-dir` requires a path")?,
                    ));
                }
                "--keep" => {
                    let value = arguments.next().context("`--keep` requires a number")?;
                    keep = value
                        .parse()
                        .with_context(|| format!("invalid `--keep` value `{value}`"))?;
                }
                "-h" | "--help" => {
                    print_help();
                    return Ok(None);
                }
                other => bail!("unknown argument `{other}`"),
            }
        }

        let output_dir = output_dir.unwrap_or_else(|| data_dir.join("backups"));

        Ok(Some(Self {
            data_dir,
            output_dir,
            keep,
        }))
    }
}

fn default_data_dir() -> PathBuf {
    PathBuf::from(DEFAULT_DATA_DIR)
}

fn optional_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn optional_keep() -> anyhow::Result<Option<usize>> {
    let Some(value) = env::var(KEEP_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    value
        .parse()
        .map(Some)
        .with_context(|| format!("invalid {KEEP_ENV} value `{value}`"))
}

fn print_help() {
    println!("Usage: cookbook-backup [OPTIONS]");
    println!();
    println!("Creates a transaction-aware SQLite backup using VACUUM INTO.");
    println!("The application can keep running while the backup is taken.");
    println!();
    println!("Options:");
    println!("  --data-dir <PATH>    Data directory containing {DATABASE_FILE_NAME}");
    println!("                       [env: {DATA_DIR_ENV}] [default: ./{DEFAULT_DATA_DIR}]");
    println!("  --output-dir <PATH>  Directory for backup files");
    println!("                       [env: {OUTPUT_DIR_ENV}] [default: <data-dir>/backups]");
    println!("  --keep <N>           Number of backups to retain");
    println!("                       [env: {KEEP_ENV}] [default: {DEFAULT_KEEP}]");
    println!("  -h, --help           Print help");
}
