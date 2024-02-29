// main.rs

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "agent")]
mod agent;

#[tokio::main]
async fn main() {
    #[cfg(feature = "server")]
    server::start_server();

    #[cfg(feature = "client")]
    client::start_client();

    #[cfg(feature = "agent")]
    client::start_agent();
}
