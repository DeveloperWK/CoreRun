mod cgroup;
mod cli;
mod error;
mod filesystem;
mod namespace;
mod network;
mod process;
mod setup;
mod volume;

use crate::{network::NetworkManager, setup::run};
use log::error;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref NETWORK_MANAGER:Arc<Mutex<NetworkManager>> = {
        Arc::new(Mutex::new(NetworkManager::new().expect("Failed to initialize network manager")))
    };
}

fn main() {
    env_logger::Builder::from_default_env()
        .format_timestamp_micros()
        .format_module_path(false)
        .filter_level(log::LevelFilter::Info)
        .init();

    if let Err(e) = run() {
        error!("Container runtime error: {e}");
        std::process::exit(1)
    }
}
