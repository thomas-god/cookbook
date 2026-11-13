use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, bail};
use cookbook_domain::recipe::ports::RecipeRepository;
use cookbook_domain::user::models::{UserId, UserRole, Username};
use cookbook_infra::auth::models::{AuthUser, UserEmail};
use cookbook_infra::auth::ports::{PasswordHasher, UserRepository};
use cookbook_infra::auth::{Argon2PasswordHasher, SqliteUserRepository};
use cookbook_infra::recipe::{
    SEED_USER_EMAIL, SEED_USER_ID, SEED_USER_PASSWORD, SEED_USER_USERNAME, SqliteRecipeRepository,
    sample_recipes,
};
use cookbook_infra::storage::SqliteDatabase;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data_dir = parse_data_dir()?;
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("failed to create the data directory {}", data_dir.display()))?;

    let database = SqliteDatabase::open(data_dir.join("cookbook.sqlite3")).await?;
    seed_user(&database).await?;
    let repository = SqliteRecipeRepository::new(database.pool().clone());

    let mut inserted = 0;
    let mut skipped = 0;

    for recipe in sample_recipes() {
        let id = recipe.id().clone();

        if repository.get_recipe(id).await?.is_some() {
            println!("skipping existing recipe `{}`", recipe.title().as_str());
            skipped += 1;
            continue;
        }

        repository
            .save_recipe_revision(recipe.recipe().clone(), recipe.revision().clone())
            .await?;
        println!("inserted recipe `{}`", recipe.title().as_str());
        inserted += 1;
    }

    println!("seed complete: {inserted} inserted, {skipped} skipped");
    Ok(())
}

async fn seed_user(database: &SqliteDatabase) -> anyhow::Result<()> {
    let users = SqliteUserRepository::new(database.pool().clone());
    let user_id = UserId::new(SEED_USER_ID.to_string());

    if users.find_by_id(&user_id).await?.is_some() {
        println!("skipping existing dev user `{SEED_USER_EMAIL}`");
        return Ok(());
    }

    let email = UserEmail::new(SEED_USER_EMAIL);
    if users.find_by_email(&email).await?.is_some() {
        bail!("the seed email `{SEED_USER_EMAIL}` already belongs to a different user");
    }

    let password_hash = Argon2PasswordHasher::new()
        .hash_password(SEED_USER_PASSWORD)
        .await?;
    let user = AuthUser::new(
        user_id,
        email,
        Some(Username::new(SEED_USER_USERNAME.to_string())),
        password_hash,
        UserRole::Admin,
    );
    users.save_user(user).await?;

    println!("inserted dev user `{SEED_USER_EMAIL}` / `{SEED_USER_PASSWORD}`");
    Ok(())
}

fn parse_data_dir() -> anyhow::Result<PathBuf> {
    let mut data_dir = env::var("COOKBOOK_DATA_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));

    let mut arguments = env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--data-dir" => {
                let value = arguments.next().context("`--data-dir` requires a path")?;
                data_dir = PathBuf::from(value);
            }
            "-h" | "--help" => {
                println!("Usage: seed_dev [--data-dir PATH]");
                println!("Defaults to COOKBOOK_DATA_DIR or ./data.");
                std::process::exit(0);
            }
            other => bail!("unknown argument `{other}`"),
        }
    }

    Ok(data_dir)
}
