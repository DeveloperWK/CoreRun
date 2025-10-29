use std::{fs, net::Ipv4Addr, process::Command};

use crate::{
    error::{ContainerError, ContainerResult},
    network::Protocol,
};

pub fn setup_nat(bridge_name: &str, subnet: &str) -> ContainerResult<()> {
    fs::write("/proc/sys/net/ipv4/ip_forward", "1").map_err(|_| ContainerError::Network {
        message: format!("Failed to enable IP forwarding"),
    })?;
    let output = Command::new("ip")
        .args(&[
            "-t",
            "nat",
            "-A",
            "POSTROUTING",
            "-s",
            subnet,
            "!",
            "-o",
            bridge_name,
            "-j",
            "MASQUERADE",
        ])
        .output()
        .map_err(|_| ContainerError::Network {
            message: format!("Failed to setup NAT"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("already exists") {
            ContainerError::Network {
                message: format!("Failed to setup NAT: {}", stderr),
            };
        }
    }
    let _ = Command::new("iptables")
        .args(&["-A", "FORWARD", "-i", bridge_name, "-j", "ACCEPT"])
        .output();
    let _ = Command::new("iptables")
        .args(&["-A", "FORWARD", "-o", bridge_name, "-j", "ACCEPT"])
        .output();
    log::info!("Setup NAT for {}", bridge_name);
    Ok(())
}
pub fn cleanup_nat(bridge_name: &str) -> ContainerResult<()> {
    let _ = Command::new("iptables")
        .args(&["-D", "FORWARD", "-i", bridge_name, "-j", "ACCEPT"])
        .output();
    let _ = Command::new("iptables")
        .args(&["-D", "FORWARD", "-o", bridge_name, "-j", "ACCEPT"])
        .output();
    Ok(())
}
pub fn add_port_forward(
    bridge_name: &str,
    host_port: u16,
    container_ip: Ipv4Addr,
    container_port: u16,
    protocol: Protocol,
) -> ContainerResult<()> {
    let proto = match protocol {
        Protocol::TCP => "tcp",
        Protocol::UDP => "udp",
    };
    let output = Command::new("iptables")
        .args(&[
            "-t",
            "nat",
            "-A",
            "PREROUTING",
            "-p",
            proto,
            "--dport",
            &host_port.to_string(),
            "-j",
            "DNAT",
            "--to-destination",
            &format!("{}:{}", container_ip, container_port),
        ])
        .output()
        .map_err(|_| ContainerError::Network {
            message: format!("Failed to add port forward"),
        })?;
    if !output.status.success() {
        ContainerError::Network {
            message: format!(
                "Failed to add port forward: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }
    let _ = Command::new("iptables")
        .args(&[
            "-A",
            "FORWARD",
            "-p",
            proto,
            "-d",
            &container_ip.to_string(),
            "--dport",
            &container_port.to_string(),
            "-j",
            "ACCEPT",
        ])
        .output();

    log::info!(
        "Port forward: {}:{} -> {}:{}",
        host_port,
        proto,
        container_ip,
        container_port
    );
    Ok(())
}

pub fn remove_port_forward(
    host_port: u16,
    container_ip: Ipv4Addr,
    container_port: u16,
    protocol: Protocol,
) -> ContainerResult<()> {
    let proto = match protocol {
        Protocol::TCP => "tcp",
        Protocol::UDP => "udp",
    };
    let output = Command::new("iptables")
        .args(&[
            "-t",
            "nat",
            "-D",
            "PREROUTING",
            "-p",
            proto,
            "--dport",
            &host_port.to_string(),
            "-j",
            "DNAT",
            "--to-destination",
            &format!("{}:{}", container_ip, container_port),
        ])
        .output()
        .map_err(|_| ContainerError::Network {
            message: format!("Failed to add port forward"),
        })?;
    if !output.status.success() {
        ContainerError::Network {
            message: format!(
                "Failed to add port forward: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }
    let _ = Command::new("iptables")
        .args(&[
            "-D",
            "FORWARD",
            "-p",
            proto,
            "-d",
            &container_ip.to_string(),
            "--dport",
            &container_port.to_string(),
            "-j",
            "ACCEPT",
        ])
        .output();

    Ok(())
}
