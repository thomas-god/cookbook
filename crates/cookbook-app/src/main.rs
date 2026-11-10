mod app;
mod components;
mod env;
mod logging;

topcoat::l10n::fluent::messages!("./l10n/{locale}.ftl");

#[tokio::main]
async fn main() {
    logging::init().expect("failed to initialize logging");

    tracing::info!("starting cookbook");
    topcoat::start(app::router().await).await.unwrap();
}
