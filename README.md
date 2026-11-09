# cookbook

Self-hosted family cookbook.

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
