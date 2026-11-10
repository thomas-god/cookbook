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

## Vision recipe import

Recipes can be imported from photos of an existing recipe using a vision-language model (VLM) via an
OpenAI-compatible model endpoint. Configure a primary endpoint and an optional fallback:

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
- Uploaded originals are kept as recipe source photos. A resized copy is generated only for the
  vision request.

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
