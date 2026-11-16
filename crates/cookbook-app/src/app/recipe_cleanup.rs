//! Scheduled cleanup of private recipes whose author no longer exists.
//!
//! Public recipes are intentionally kept when a user is deleted. Only private
//! recipes become unreachable, so they are physically removed by a background
//! sweep. The sweep runs once at startup and then hourly.

use std::time::Duration;

use cookbook_domain::recipe::ports::RecipeService;
use tokio::time::MissedTickBehavior;

use crate::app::AppRecipeService;

const CLEANUP_INTERVAL: Duration = Duration::from_hours(1);

/// Starts the background orphaned-recipe cleanup task.
pub fn spawn(service: AppRecipeService) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            match service.purge_orphaned_private_recipes().await {
                Ok(0) => {}
                Ok(deleted) => {
                    tracing::info!(deleted, "purged orphaned private recipes");
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to purge orphaned private recipes");
                }
            }
        }
    });
}
