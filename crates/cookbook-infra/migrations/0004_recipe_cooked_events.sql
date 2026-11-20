-- Per-user cooking log entries. Events keep the recipe title and revision as a
-- snapshot so they remain meaningful after the recipe is deleted.

CREATE TABLE recipe_cooked_events (
    id TEXT PRIMARY KEY,
    -- Nullable so deleting a recipe keeps the event and its snapshot.
    recipe_id TEXT REFERENCES recipes (id) ON DELETE SET NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    recipe_revision_id TEXT NOT NULL,
    recipe_title TEXT NOT NULL,
    cooked_on TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX recipe_cooked_events_recipe_user_idx
    ON recipe_cooked_events (recipe_id, user_id, cooked_on DESC);

CREATE INDEX recipe_cooked_events_user_idx
    ON recipe_cooked_events (user_id, cooked_on DESC);
