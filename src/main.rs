// main.rs
mod operation;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "agent")]
mod agent;

#[tokio::main]
async fn main() {
    #[cfg(feature = "server")]
    server::start_server().await;

    #[cfg(feature = "client")]
    client::start_client().await;

    #[cfg(feature = "agent")]
    agent::start_agent().await;
}
