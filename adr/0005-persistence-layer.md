# Persistent storage

- Status: Accepted
- Date: 2026-11-13

## Context

Recipe content is a nested value object with revisions, while images and source documents are opaque
byte payloads. The storage layer must preserve revision history and keep asset bytes out of the
relational database.

## Decisions

### SQLite for structured data

Users, sessions, recipes, revisions, and asset metadata live in one SQLite database. The adapter
uses `sqlx` with a connection pool:

- four connections for file-backed databases,
- WAL journal mode,
- `synchronous=NORMAL`,
- foreign-key enforcement enabled.

Writes use `BEGIN IMMEDIATE` so a deferred transaction cannot fail while upgrading to a write lock.
SQLite still allows only one writer at a time; the pool improves read concurrency and allows reads
while a write transaction is open.

### Dumb data layer

The database does not enforce business rules through its schema:

- no `CHECK` constraints for enum-like values,
- no enum lookup tables,
- no `UNIQUE` constraints for business identity such as email addresses,
- no foreign keys on user references.

Values such as recipe scope, user role, revision verification, and category are stored as plain
text. Email uniqueness and author/scope immutability remain service and repository-contract rules.

Structural constraints are still used: primary keys, indexes, and foreign keys for recipe-owned
rows.

### Recipe-owned rows cascade

`recipe_revisions`, `recipe_images`, and `recipe_source_documents` reference `recipes(id)` with
`ON DELETE CASCADE`. Deleting a recipe removes its revision history and asset metadata.

There are deliberately no foreign keys on `recipes.author_id`, `sessions.user_id`,
`recipe_revisions.editor_id`, or the `added_by` columns. Deleting a user does not delete recipes or
sessions at the repository layer.

### Revisions as versioned JSON snapshots

Each revision stores a complete `RecipeContent` snapshot as JSON, plus a `content_version` column.
The JSON is the source of truth for content. Decoding bypasses domain constructors so ingredient
normalization and validation are not re-applied to stored snapshots.

The current version is `1`. Old revisions keep their original version and are decoded through a
version dispatcher. A breaking content change must add a new decode path and bump the version; old
revisions are not rewritten. Hand-written golden fixtures freeze the v1 JSON shape and fail if the
serialized format changes unintentionally.

A few columns are derived from the content at write time for querying:

- `title` and `title_key` for display and case-insensitive sorting,
- `category` for category filters,
- `verification` for excluding recipes that need review,
- `search_text` for substring search over title, description, ingredients, steps, and parts.

These columns are query helpers, not constraints. If search semantics change, they must be rebuilt
from the stored JSON.

### Files for asset bytes

Image and source-document bytes are stored on disk, grouped by recipe:

```text
<data_dir>/recipes/<recipe_id>/images/<image_id>
<data_dir>/recipes/<recipe_id>/documents/<document_id>
```

SQLite stores the metadata and a relative `storage_key`. Writes use a temporary file, `sync_all`,
and rename so readers never observe a partial file. Recipe and asset ids are validated as safe path
components before use.

Adding assets writes files first and then inserts metadata in a transaction; written files are
removed if the insert fails. Deleting a source document removes metadata first and then the file. A
crash can leave orphan files, which are harmless and can be cleaned up later.

### Recipe deletion

Only the recipe author may delete a recipe. The service deletes the recipe row, which cascades to
revisions and asset metadata, then asks the document repository to remove the recipe directory.
Asset cleanup failure is logged after the database deletion has succeeded.

### Configuration and startup

`COOKBOOK_DATA_DIR` selects the data directory and defaults to `./data`. The directory is created at
startup, migrations run before the server starts, and the Docker image uses `/data` as a declared
volume.

The in-memory adapters remain available for tests. Production startup never seeds sample data.

### Development seeding

A separate `seed_dev` binary inserts a dev user and the sample recipes. It is idempotent, skips
existing rows, and is never called by the server. The dev user is `dev@example.com` with password
`cookbook-dev` and owns the sample recipes.

## Consequences

### Benefits

- Revision history and asset bytes have clear, separate homes.
- WAL allows concurrent reads and reads during writes.
- The schema does not duplicate domain validation rules.
- Content format changes are explicit and versioned.
- Backups are a single directory copy when the app is stopped.

### Tradeoffs

- SQLite has a single writer; write-heavy workloads will serialize.
- The database and `recipes/` directory must be backed up together.
- JSON snapshots are coupled to the serde representation of `RecipeContent`.
- Derived search columns must be rebuilt if search semantics change.
- Orphan files are possible after a crash between file write and metadata insert.
- The file store assumes a local filesystem and a single application instance.

### When to revisit

- Add a cleanup command for orphan files.
- Add FTS5 or another search index if substring search over `search_text` becomes too slow.
- Add a content migration when the v1 JSON shape needs a breaking change.
