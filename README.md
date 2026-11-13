# cookbook

Self-hosted family cookbook.

## Authentication

Authentication is email + password. Usernames are optional and only used for display.

Self-registration is disabled by default. Enable it with:

```sh
COOKBOOK_REGISTRATION_ENABLED=true
```

To create the first user as an admin, set the following variables before the first start. They are
ignored once at least one user exists:

```sh
COOKBOOK_FIRST_ADMIN_EMAIL=admin@example.com
COOKBOOK_FIRST_ADMIN_PASSWORD='a-long-password'
COOKBOOK_FIRST_ADMIN_USERNAME=Admin   # optional
```

Session cookie security follows the `X-Forwarded-Proto` header:

- `https` → a hardened `__Host-session` cookie with `Secure`
- anything else → a regular `session` cookie without `Secure`, for trusted networks such as
  Tailscale or a private LAN

A TLS-terminating reverse proxy must set `X-Forwarded-Proto`. If the app is exposed directly to
untrusted clients, make sure they cannot spoof that header.

## Persistence

Data is stored under `COOKBOOK_DATA_DIR`, which defaults to `./data`:

```text
data/
  cookbook.sqlite3
  recipes/
    <recipe_id>/
      images/<image_id>
      documents/<document_id>
```

SQLite holds users, sessions, recipes, revisions, and asset metadata. Image and source-document
bytes live in the per-recipe directories. Migrations run automatically at startup.

The Docker image uses `/data` and declares it as a volume. Mount it to keep data across container
replacements:

```sh
docker run -v cookbook-data:/data -p 3000:3000 ghcr.io/thomas-god/cookbook
```

## Vision recipe import

Recipes can be imported from photos or a PDF of an existing recipe using a vision-language model
(VLM) via an OpenAI-compatible model endpoint. Configure a primary endpoint and an optional
fallback:

```sh
COOKBOOK_VLM_PRIMARY_URL=http://localhost:1234/v1/chat/completions
COOKBOOK_VLM_PRIMARY_MODEL=qwen2.5-vl-7b-instruct
COOKBOOK_VLM_PRIMARY_API_KEY=        # optional for local endpoints
COOKBOOK_VLM_PRIMARY_TIMEOUT_SECONDS=180

COOKBOOK_VLM_BACKUP_URL=https://openrouter.ai/api/v1/chat/completions
COOKBOOK_VLM_BACKUP_MODEL=qwen/qwen2.5-vl-72b-instruct
COOKBOOK_VLM_BACKUP_API_KEY=sk-or-...
COOKBOOK_VLM_BACKUP_TIMEOUT_SECONDS=120
```

- No endpoint configured disables the import feature and hides it from the UI.
- The backup endpoint can be configured without a primary endpoint.
- The app tries endpoints in order: primary first, then backup.
- URLs are full chat-completions URLs, not just base URLs.
- Requests use structured output (`response_format.json_schema`).
- Uploaded originals are kept as recipe source documents. A resized copy is generated only for the
  vision request.
- A PDF is imported as a single recipe and may contain at most 10 pages. Each page is rendered to an
  image for the vision request only; the original PDF is stored unchanged and linked from the
  recipe.
- A PDF cannot be imported together with photos or another PDF.
- Password-protected PDFs are not supported.

## Secrets

Every environment variable `NAME` can also be read from a file by setting `NAME_FILE` to the file
path. This works with
[Docker secrets](https://docs.docker.com/engine/swarm/secrets/#build-support-for-docker-secrets-into-your-images)
or any mounted secret:

```sh
COOKBOOK_FIRST_ADMIN_PASSWORD_FILE=/run/secrets/admin_password
COOKBOOK_VLM_PRIMARY_API_KEY_FILE=/run/secrets/vlm_api_key
```

The file takes precedence over the direct variable. Surrounding whitespace, including the trailing
newline added by many secret stores, is trimmed. If `NAME_FILE` is set but the file cannot be read,
startup fails instead of silently falling back.

## Development

Run the app with the Topcoat development server:

```sh
topcoat dev
```

To run the app outside the dev server, build the asset bundle first:

```sh
topcoat asset bundle
cargo run
```

### Dev seed data

Seed the dev user and sample recipes into the configured data directory:

```sh
./scripts/seed-dev.sh
# or
cargo run -p cookbook-infra --bin seed_dev -- --data-dir ./data
```

The dev user is `dev@example.com` / `test` and owns the sample recipes. Seeding is idempotent and is
never run automatically by the server.

### Persisted JSON fixtures

Recipe content and revision origins are stored as versioned JSON. Their current wire format is
frozen by golden fixtures in `crates/cookbook-domain/tests/fixtures/` and the tests in
`crates/cookbook-domain/tests/content_v1.rs`:

```sh
cargo test -p cookbook-domain --features serde --test content_v1
```

The fixtures are written by hand, not generated from the domain types. A failing golden test means
the persisted format changed. Do not update the fixture in place: add a new version, keep the old
fixture decodable, and bump the content version.

### UI components

We use Topcoat UI components that are vendored/installed directly into the application crate, so
they can be edited like any other source file. To install, update or remove component use the
[topcoat CLI](https://github.com/tokio-rs/topcoat/blob/main/docs/ui.md) from `crates/cookbook-app`:

```sh
cd crates/cookbook-app
topcoat ui list
topcoat ui list --installed
topcoat ui add dialog
topcoat ui remove dialog
```

### Translations

The UI is translated with Topcoat's Fluent integration. Message catalogs live in
`crates/cookbook-app/src/l10n/{locale}.ftl`.

The request locale is selected from the `locale` cookie, then the browser's `Accept-Language`
header, and falls back to English. To add a language, add its `.ftl` file, add its endonym to
`locale_name` in `src/app/locale_select.rs`, and run:

```sh
cd crates/cookbook-app
topcoat l10n check
```
