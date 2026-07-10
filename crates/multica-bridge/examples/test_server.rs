//! Multica Test Server
//!
//! Starts a complete test server that implements the Multica protocol.

use multica_bridge::*;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Multica Test Server ===");
    println!("Starting test server on 0.0.0.0:8080");
    println!("Press Ctrl+C to stop\n");

    if let Err(e) = start_test_server("0.0.0.0:8080").await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
