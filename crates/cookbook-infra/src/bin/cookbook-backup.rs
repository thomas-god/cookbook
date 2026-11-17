use std::path::PathBuf;

use anyhow::{Context, bail};
use cookbook_config::{defaults, env, names};
use cookbook_infra::storage::{DataConfig, backup_database};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let options = match Options::from_env_and_args()? {
        Some(options) => options,
        None => return Ok(()),
    };

    let source = options.data_dir.join(defaults::DATABASE_FILE_NAME);
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
        let mut data_dir =
            optional_path(names::data::DIR)?.unwrap_or_else(|| PathBuf::from(defaults::DATA_DIR));
        let mut output_dir = optional_path(names::backup::DIR)?;
        let mut keep = optional_keep()?.unwrap_or(defaults::BACKUP_KEEP);

        let mut arguments = std::env::args().skip(1);

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

        let output_dir = output_dir.unwrap_or_else(|| DataConfig::new(&data_dir).backup_dir());

        Ok(Some(Self {
            data_dir,
            output_dir,
            keep,
        }))
    }
}

fn optional_path(name: &str) -> anyhow::Result<Option<PathBuf>> {
    Ok(env::optional(name)?.map(PathBuf::from))
}

fn optional_keep() -> anyhow::Result<Option<usize>> {
    let Some(value) = env::optional(names::backup::KEEP)? else {
        return Ok(None);
    };

    value
        .parse()
        .map(Some)
        .with_context(|| format!("invalid {} value `{value}`", names::backup::KEEP))
}

fn print_help() {
    println!("Usage: cookbook-backup [OPTIONS]");
    println!();
    println!("Creates a transaction-aware SQLite backup using VACUUM INTO.");
    println!("The application can keep running while the backup is taken.");
    println!();
    println!("Options:");
    println!(
        "  --data-dir <PATH>    Data directory containing {}",
        defaults::DATABASE_FILE_NAME
    );
    println!(
        "                       [env: {}] [default: ./{}]",
        names::data::DIR,
        defaults::DATA_DIR
    );
    println!("  --output-dir <PATH>  Directory for backup files");
    println!(
        "                       [env: {}] [default: <data-dir>/{}]",
        names::backup::DIR,
        defaults::BACKUP_DIR_NAME
    );
    println!("  --keep <N>           Number of backups to retain");
    println!(
        "                       [env: {}] [default: {}]",
        names::backup::KEEP,
        defaults::BACKUP_KEEP
    );
    println!("  -h, --help           Print help");
}
