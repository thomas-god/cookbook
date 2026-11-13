-- Initial persistence schema.

CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    username TEXT,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL
);

CREATE INDEX users_email_idx ON users (email);

CREATE TABLE sessions (
    token_hash BLOB PRIMARY KEY,
    user_id TEXT NOT NULL,
    expires_at_ms INTEGER NOT NULL
);

CREATE TABLE recipes (
    id TEXT PRIMARY KEY,
    author_id TEXT NOT NULL,
    scope TEXT NOT NULL
);

CREATE TABLE recipe_revisions (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL,
    recipe_id TEXT NOT NULL REFERENCES recipes (id) ON DELETE CASCADE,
    editor_id TEXT NOT NULL,
    edited_at_ms INTEGER NOT NULL,
    origin TEXT NOT NULL,
    verification TEXT NOT NULL,
    content_version INTEGER NOT NULL,
    content TEXT NOT NULL,
    -- derived columns from content, used for search and/or sort
    title TEXT NOT NULL,
    title_key TEXT NOT NULL,
    category TEXT,
    search_text TEXT NOT NULL
);

CREATE INDEX recipe_revisions_recipe_seq_idx ON recipe_revisions (recipe_id, seq);
CREATE INDEX recipe_revisions_recipe_edited_idx ON recipe_revisions (recipe_id, edited_at_ms);
CREATE INDEX recipe_revisions_title_key_idx ON recipe_revisions (title_key);

CREATE TABLE recipe_images (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL,
    recipe_id TEXT NOT NULL REFERENCES recipes (id) ON DELETE CASCADE,
    content_type TEXT NOT NULL,
    byte_len INTEGER NOT NULL,
    added_by TEXT NOT NULL,
    added_at_ms INTEGER NOT NULL,
    storage_key TEXT NOT NULL
);

CREATE INDEX recipe_images_recipe_seq_idx ON recipe_images (recipe_id, seq);
CREATE INDEX recipe_images_recipe_id_idx ON recipe_images (recipe_id, id);

CREATE TABLE recipe_source_documents (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL,
    recipe_id TEXT NOT NULL REFERENCES recipes (id) ON DELETE CASCADE,
    content_type TEXT NOT NULL,
    original_name TEXT,
    byte_len INTEGER NOT NULL,
    added_by TEXT NOT NULL,
    added_at_ms INTEGER NOT NULL,
    storage_key TEXT NOT NULL
);

CREATE INDEX recipe_source_documents_recipe_seq_idx
    ON recipe_source_documents (recipe_id, seq);
CREATE INDEX recipe_source_documents_recipe_id_idx
    ON recipe_source_documents (recipe_id, id);
