mod cmd;

use clap::Parser;
use cmd::root::{RootCmd, handle_root};
use rsdish::web::logs::WebLogLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() {
    let fmt_layer = tracing_subscriber::fmt::layer();

    // The web layer keeps a bounded in-memory copy of every event so the web UI
    // can show them. It costs nothing when no web UI is running.
    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(fmt_layer)
        .with(WebLogLayer::new())
        .init();

    let cli = RootCmd::parse();
    println!("{:#?}", cli);
    handle_root(cli);
}
