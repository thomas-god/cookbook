mod app;
mod components;

topcoat::l10n::fluent::messages!("./l10n/{locale}.ftl");

#[tokio::main]
async fn main() {
    topcoat::start(app::router()).await.unwrap();
}
