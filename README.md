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
