# cookbook

**Cookbook** is a self-hosted web-application to manage and share your family recipes. Recipes can
be either created from scratch or imported from images (cookbook pages, handwritten recipes, etc.)
via an optional VLM-based vision module. The app supports public/private recipes, recipe revisions
history, a big-picture cook mode, and more.

## Table of contents

- [Try it locally with Docker](#try-it-locally-with-docker)
- [Deployment with Docker compose/stack](#deployment-with-docker-composestack)
  - [Persistence](#persistence)
  - [Backups](#backups)
  - [Users registration and authentication](#users-registration-and-authentication)
  - [Vision module (optional)](#vision-module-optional)
  - [List of environment variables](#list-of-environment-variables)
- [Local development](#local-development)

## Try it locally with Docker

To quickly test the application try it locally with Docker by running the following command and then
visiting http://localhost:3000:

```bash
docker run --rm \
  --name cookbook_demo
  -e COOKBOOK_REGISTRATION_ENABLED=true \
  -v cookbook_data:/data \
  ghcr.io/thomas-god/cookbook:latest
```

You can seed the application's data with some test recipes by running:

```bash
docker exec cookbook_demo /app/cookbook-seed
```

## Deployment with Docker compose/stack

`Cookbook` is distributed as Docker image for `amd64` and `arm64` architectures. You can use docker
compose or stack to deploy it.

```yaml
services:
  cookbook:
    image: ghcr.io/thomas-god/cookbook:latest
    environment:
      COOKBOOK_REGISTRATION_ENABLED: "true"
    volumes:
      - cookbook_data:/data

volumes:
  cookbook_data:
```

### Persistence

The application uses `/data` to store its in-container SQLite database (user and recipe data) and
assets (recipes images). Mount a Docker volume or an host path to this location to persist data
accros container restarts.

### Backups

Automatic background backups are enabled by default: the backup process creates a consistent,
compacted SQLite snapshot in `/data/backups`. The application does not need to be stopped during
backups.

| Variable                   | Default              | Meaning                           |
| -------------------------- | -------------------- | --------------------------------- |
| `COOKBOOK_BACKUP_ENABLED`  | `true`               | Enable automatic backups          |
| `COOKBOOK_BACKUP_SCHEDULE` | `0 3 * * *`          | Five-field cron expression in UTC |
| `COOKBOOK_BACKUP_DIR`      | `<data_dir>/backups` | Backup output directory           |
| `COOKBOOK_BACKUP_KEEP`     | `14`                 | Number of backups files to retain |

Backup files are named `cookbook-YYYYMMDDTHHMMSSZ.sqlite3` using the time of backup.

The default location is inside the data volume. For protection against losing that volume, mount a
separate volume and point `COOKBOOK_BACKUP_DIR` at it.

Backups contain only the database. Image and source-document bytes under `data/recipes/` must be
backed up separately, for example with a filesystem snapshot, `restic`, or `rsync`. Restoring the
database alone does not restore the asset files.

#### Manual backups

You can create a manual backup at any time using the in-container CLI:

```sh
docker exec <container_name> /app/cookbook-backup
```

#### Backup restoration

To restore a database backup:

- stop the cookbook container,
- remove or rename the live database and its WAL companions (`/data/cookbook.sqlite3`,
  `/data/cookbook.sqlite3-wal` and `/data/cookbook.sqlite3-shm`),
- copy the backup file `cookbook-YYYYMMDDTHHMMSSZ.sqlite3` into `/data`
- rename the backup file to `cookbook.sqlite3`,
- restart the container.

### Users registration and authentication

Users can register using an email/password pair. Email addresses are not verified as the app does
not ship a mailer. User account recovery is made via the [admin role](#admin-role).

Open registration can be disabled via `COOKBOOK_REGISTRATION_ENABLED=false`.
[Admin users](#admin-role) can still create new users when registration is disabled.

#### Admin role

Admin users can manage other users: create new user, update email and password, promote/demote users
as admin, and delete user. When you create a new instance it can be initialized with an admin user
using the following environment variables:

```bash
COOKBOOK_FIRST_ADMIN_EMAIL=you@example.com
COOKBOOK_FIRST_ADMIN_PASSWORD='a-long-password'
COOKBOOK_FIRST_ADMIN_USERNAME=you
```

This is a no-op if the instance already contains some users (admin or not). To avoid having password
in your config file please refer to this [section](#loading-secrets).

For emergency recovery the application ships an in-container recovery CLI that can be used to set a
user's password or promote an user to admin:

```bash
docker exec <container_name> /app/cookbook-admin --help
```

#### HTTPS is recommended, plain HTTP is still possible on a trusted network

The bundled image only serves plain HTTP and does not terminate TLS by itself. Using a
TLS-terminating reverse proxy (e.g. [Caddy](https://caddyserver.com/) or
[Traefik](https://traefik.io/)) is recommended.

Over plain HTTP login works, but then the session cookie travels in clear. Only use HTTP when the
traffic is already protected by another layer, for example a [WireGuard](https://www.wireguard.com/)
or [Tailscale](https://tailscale.com/) tunnel.

A minimal `docker-compose.yaml` with Traefik handling TLS looks like:

```yaml
services:
  cookbook:
    image: ghcr.io/thomas-god/cookbook:latest
    environment:
      COOKBOOK_REGISTRATION_ENABLED: "true"
    volumes:
      - cookbook_data:/data
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.cookbook.rule=Host(`your.domain`)"
      - "traefik.http.routers.cookbook.entrypoints=websecure"
      - "traefik.http.routers.cookbook.tls.certresolver=letsencrypt"
      - "traefik.http.services.cookbook.loadbalancer.server.port=3000"

  traefik:
    image: traefik:v3.2
    command:
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.web.address=:80"
      - "--entrypoints.websecure.address=:443"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web"
      - "--certificatesresolvers.letsencrypt.acme.email=you@your.domain"
      - "--certificatesresolvers.letsencrypt.acme.storage=/letsencrypt/acme.json"
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - traefik_letsencrypt:/letsencrypt

volumes:
  cookbook_data:
  traefik_letsencrypt:
```

### Vision module (optional)

`Cookbook` has an optional vision module that can import recipes from images and documents (jpeg,
png and pdf) using any vision-language model (VLM) model via OpenAI-compatible model endpoint. You
can configure a primary provider (e.g. a local instance) and an optional fallback one (e.g.
OpenRouter). If no provider is setup the vision-module is disabled.

```bash
COOKBOOK_VLM_PRIMARY_URL=http://localhost:1234/v1/chat/completions
COOKBOOK_VLM_PRIMARY_MODEL=mistralai/ministral-3b-2512
COOKBOOK_VLM_PRIMARY_API_KEY=        # optional for local endpoints
COOKBOOK_VLM_PRIMARY_TIMEOUT_SECONDS=180

COOKBOOK_VLM_BACKUP_URL=https://openrouter.ai/api/v1/chat/completions
COOKBOOK_VLM_BACKUP_MODEL=mistralai/ministral-3b-2512
COOKBOOK_VLM_BACKUP_API_KEY=sk-or-...
COOKBOOK_VLM_BACKUP_TIMEOUT_SECONDS=120
```

Model/provider must support
[structured output](https://lmstudio.ai/docs/developer/openai-compat/structured-output#structured-output).

#### System prompt

If you need to tweak vision import for a specific model and/or sources you can replace the default
[system prompt](crates/cookbook-vision/prompts/recipe-system.txt) by using a global prompt
(`COOKBOOK_VLM_SYSTEM_PROMPT_FILE`) or a prompt per provider
(`COOKBOOK_VLM_PRIMARY_SYSTEM_PROMPT_FILE` and `COOKBOOK_VLM_BACKUP_SYSTEM_PROMPT_FILE`).

### List of environment variables

The following variables are available to configure the application.

| Variable                               | Default              | Description                                                                       |
| -------------------------------------- | -------------------- | --------------------------------------------------------------------------------- |
| `COOKBOOK_REGISTRATION_ENABLED`        | `false`              | Allow public user registration                                                    |
| `COOKBOOK_FIRST_ADMIN_EMAIL`           | unset                | Email of the initial admin; requires `COOKBOOK_FIRST_ADMIN_PASSWORD`              |
| `COOKBOOK_FIRST_ADMIN_PASSWORD`        | unset                | Password of the initial admin; requires `COOKBOOK_FIRST_ADMIN_EMAIL`              |
| `COOKBOOK_FIRST_ADMIN_USERNAME`        | unset                | Optional display name for the initial admin; requires email and password          |
| `COOKBOOK_BACKUP_ENABLED`              | `true`               | Enable automatic database backups                                                 |
| `COOKBOOK_BACKUP_SCHEDULE`             | `0 3 * * *`          | Five-field cron expression in UTC                                                 |
| `COOKBOOK_BACKUP_DIR`                  | `<data_dir>/backups` | Backup output directory                                                           |
| `COOKBOOK_BACKUP_KEEP`                 | `14`                 | Number of backup files to retain (at least 1)                                     |
| `COOKBOOK_VLM_PRIMARY_URL`             | unset                | OpenAI-compatible primary endpoint; setting it enables the primary provider       |
| `COOKBOOK_VLM_PRIMARY_MODEL`           | unset                | Model name for the primary endpoint; required with `COOKBOOK_VLM_PRIMARY_URL`     |
| `COOKBOOK_VLM_PRIMARY_API_KEY`         | unset                | API key for the primary endpoint; optional for local endpoints                    |
| `COOKBOOK_VLM_PRIMARY_TIMEOUT_SECONDS` | `180`                | Request timeout for the primary endpoint in seconds                               |
| `COOKBOOK_VLM_BACKUP_URL`              | unset                | OpenAI-compatible fallback endpoint; setting it enables the backup provider       |
| `COOKBOOK_VLM_BACKUP_MODEL`            | unset                | Model name for the fallback endpoint; required with `COOKBOOK_VLM_BACKUP_URL`     |
| `COOKBOOK_VLM_BACKUP_API_KEY`          | unset                | API key for the fallback endpoint; optional for endpoints that do not require one |
| `COOKBOOK_VLM_BACKUP_TIMEOUT_SECONDS`  | `120`                | Request timeout for the fallback endpoint in seconds                              |
| `COOKBOOK_VLM_SYSTEM_PROMPT`           | unset                | System prompt for every endpoint; replaces the built-in prompt                    |
| `COOKBOOK_VLM_PRIMARY_SYSTEM_PROMPT`   | unset                | System prompt for the primary endpoint; overrides the global prompt               |
| `COOKBOOK_VLM_BACKUP_SYSTEM_PROMPT`    | unset                | System prompt for the fallback endpoint; overrides the global prompt              |

#### Loading secrets

Every environment variable `NAME` can also be read from a file by setting `NAME_FILE` to the file
path. This works with
[Docker secrets](https://docs.docker.com/engine/swarm/secrets/#build-support-for-docker-secrets-into-your-images)
or any mounted secret:

```sh
COOKBOOK_FIRST_ADMIN_PASSWORD_FILE=/run/secrets/admin_password
COOKBOOK_VLM_PRIMARY_API_KEY_FILE=/run/secrets/vlm_api_key
```

The file takes precedence over the direct variable. If `NAME_FILE` is set but the file cannot be
read, startup fails instead of silently falling back.

## Local development

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
cargo run -p cookbook-infra --bin cookbook-seed -- --data-dir ./data
```

The dev user is `test@example.com` / `test` and owns the sample recipes. Seeding is idempotent.

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
