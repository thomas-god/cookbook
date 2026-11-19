-- Per-user, private recipe favorites.

CREATE TABLE recipe_favorites (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    recipe_id TEXT NOT NULL REFERENCES recipes (id) ON DELETE CASCADE,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (user_id, recipe_id)
);

CREATE INDEX recipe_favorites_user_idx ON recipe_favorites (user_id);
