use std::process::Command;

use log::info;

use crate::{
    NETWORK_MANAGER,
    cli::ContainerConfig,
    error::{ContainerError, ContainerResult},
    network,
};

pub fn setup_container_network_parent(
    container_id: &str,
    child_pid: i32,
    config: &ContainerConfig,
) -> ContainerResult<()> {
    info!("Setting up network for container from parent process...");
    let network_mode = config.network_mode.clone();
    let ports = config.ports.clone();
    let network_manager = NETWORK_MANAGER.lock().unwrap();
    let container_network = network_manager
        .setup_container_network(container_id, child_pid, network_mode, ports)
        .map_err(|e| ContainerError::Network {
            message: format!("Failed to setup network: {}", e),
        })?;
    if let Some(ip) = container_network.ip_address {
        info!("Container IP address: {}", ip);
    }
    if let Some(gw) = container_network.gateway {
        info!("Container gateway: {}", gw);
    }
    for port in &container_network.ports {
        info!(
            "Port mapping: {}:{} -> container:{}",
            port.host_port,
            match port.protocol {
                network::Protocol::TCP => "tcp",
                network::Protocol::UDP => "udp",
            },
            port.container_port
        );
    }
    Ok(())
}
pub fn cleanup_container_network(container_id: &str) -> ContainerResult<()> {
    info!("Cleaning up network for container...");
    let network_manager = NETWORK_MANAGER.lock().unwrap();
    network_manager
        .cleanup_container_network(container_id)
        .map_err(|e| ContainerError::Network {
            message: format!("Failed to cleanup network: {}", e),
        })?;
    let _ = Command::new("iptables")
        .args([
            "-t",
            "nat",
            "-D",
            "POSTROUTING",
            "-s",
            "127.0.0.1",
            "-d",
            "127.17.0.0/16",
            "-j",
            "MASQUERADE",
        ])
        .output();
    Ok(())
}
