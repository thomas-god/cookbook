mod app;
mod components;
mod env;

topcoat::l10n::fluent::messages!("./l10n/{locale}.ftl");

#[tokio::main]
async fn main() {
    topcoat::start(app::router().await).await.unwrap();
}
