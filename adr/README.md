# Architecture Decision Records

## ADRs

1. [Client-side collection state, form data and wire format](0001-client-side-collections-and-form-wire-format.md)
2. [Recipe revision history](0002-recipe-revision-history.md)
3. [Reorderable and splittable recipe steps](0003-reorderable-steps.md)
4. [Multipart recipes](0004-multipart-recipes.md)
5. [Persistent storage](0005-persistence-layer.md)
6. [Break-glass administration CLI](0006-break-glass-cli.md)
7. [User deletion and recipe cleanup](0007-user-deletion-recipe-cleanup.md)

## Tech stack

- **Language/runtime:** Rust 2024, Tokio async runtime.
- **Web framework:** [Topcoat](https://github.com/tokio-rs/topcoat) 0.12 — server-rendered HTML with
  `view!` templates, typed routing, layers, and a small runtime for interactive shards (signals,
  bindings, and procedures for add/remove/reorder operations).
- **UI:** Topcoat UI components, Tailwind CSS, Geist font, Iconify icons, Fluent localization
  (`en`/`fr`), and a small hand-written `assets/app.js` for dialogs, menus, and form polish.
- **Storage:** SQLite via `sqlx` (bundled, WAL mode) for users, sessions, recipes, revisions, and
  asset metadata; recipe images and source documents are stored as files on disk.
- **Vision (optional):** `cookbook-vision` validates/prepares images and PDFs and calls one or more
  OpenAI-compatible chat-completions endpoints with failover, parsing the model output into a recipe
  draft.
- **Operations:** scheduled SQLite online backups (`cron`), background cleanup, structured `tracing`
  logs, and separate `cookbook-admin`, `cookbook-backup`, and `cookbook-seed` binaries.
- **Packaging:** single Docker image (multi-stage build) for `amd64`/`arm64`, with `/data` as the
  persistent volume.

## Overall architecture

The application is a Rust workspace split by responsibility, with dependencies pointing inward
(inspired from hexagonal architecture):

```
   ├── cookbook-domain  (entities, value objects, service logic, ports)
   ├── cookbook-infra   (SQLite/file adapters, auth adapters, migrations)
   ├── cookbook-vision  (image/PDF → recipe via VLM)
   ├── cookbook-export  (recipe → PDF)
   └── cookbook-config  (environment variable names, defaults, parsing)
   └── cookbook-app     (web/UI, routes, layers, background tasks, binaries wiring)
```

### Request flow

1. A request enters the Topcoat router, which runs layers for request logging, locale selection, and
   authentication. The authentication layer resolves the session cookie to an `AuthenticatedUser` in
   the request context and renews sessions when needed.
2. Route handlers render server-side views, validate form submissions, and call an application
   service.
3. Application services (`RecipeServiceImpl`, `AuthServiceImpl`) are generic over their ports. The
   domain holds business rules — recipe revisions, scope checks, stale-form detection, structured
   content validation — and depends only on traits, not on SQLite or files.
4. Infrastructure adapters implement those traits: `SqliteRecipeRepository`, `SqliteUserRepository`,
   `SqliteSessionRepository`, `FileRecipeDocumentRepository`, and `FileStore`. Constructors wire the
   concrete types in `cookbook-app/src/app.rs`.

### Data model

- `recipes` holds identity, author, and scope (public/private).
- `recipe_revisions` is append-only: every content change creates a revision with editor, time,
  origin, verification state, and the full content as versioned JSON. The current recipe is the
  latest revision; derived columns (`title`, `category`, `search_text`) support search and sorting.
- Images and source documents are stored as blobs on disk with metadata rows in SQLite, so asset
  bytes never live in the database.
- Content JSON is versioned (`CURRENT_CONTENT_VERSION`) so old revisions remain readable after the
  schema evolves.

### Cross-cutting concerns

- **Background work:** Tokio tasks run scheduled database backups and an hourly sweep that deletes
  private recipes whose author no longer exists. Both are best-effort and only log failures.
- **Configuration:** every environment variable name and default lives in `cookbook-config`, shared
  by the web app and CLI tools, with `*_FILE` support for Docker secrets.
- **Observability:** `tracing` with an environment-driven filter; requests log method, path, status,
  latency, and errors.
- **Authorization:** route-level helpers (`require_user`, `require_admin`) plus domain-level scope
  checks. The only invariant-protecting operator path is the break-glass CLI (ADR 0006).
