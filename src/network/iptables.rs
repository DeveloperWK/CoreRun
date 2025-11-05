use std::{fs, net::Ipv4Addr, process::Command};

use crate::{
    error::{ContainerError, ContainerResult},
    network::Protocol,
};

pub fn setup_nat(bridge_name: &str, subnet: &str) -> ContainerResult<()> {
    log::info!("Enabling IP forwarding...");

    match fs::write("/proc/sys/net/ipv4/ip_forward", "1") {
        Ok(_) => log::info!("IP forwarding enabled via /proc"),
        Err(e) => {
            log::warn!("Failed to write to /proc: {}", e);
            let output = Command::new("sysctl")
                .args(["-w", "net.ipv4.ip_forward=1"])
                .output()?;
            if !output.status.success() {
                ContainerError::Network {
                    message: format!(
                        "Failed to enable IP forwarding: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                };
            }
            log::info!("IP forwarding enabled via sysctl");
        }
    }
    let forwarding = fs::read_to_string("/proc/sys/net/ipv4/ip_forward")
        .unwrap_or_default()
        .trim()
        .to_string();
    if forwarding != "1" {
        ContainerError::Network {
            message: format!("IP forwarding is not enabled (value: {})", forwarding),
        };
    }
    log::info!("IP forwarding verified: enabled");
    log::info!(
        "Setting up MASQUERADE rule for {} -> {}",
        subnet,
        bridge_name
    );

    let output = Command::new("iptables")
        .args([
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
            message: "Failed to setup NAT".to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stderr.contains("already exists") || stderr.contains("Duplicate") {
            log::info!("MASQUERADE rule already exists");
        } else {
            log::error!("iptables MASQUERADE failed!");
            log::error!("stderr: {}", stderr);
            log::error!("stdout: {}", stdout);
            ContainerError::Network {
                message: format!("Failed to setup MASQUERADE: {}", stderr),
            };
        }
    } else {
        log::info!("MASQUERADE rule added successfully");
    }
    log::info!("Adding FORWARD rules for {}", bridge_name);
    let output = Command::new("iptables")
        .args(["-I", "FORWARD", "1", "-i", bridge_name, "-j", "ACCEPT"])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to add FORWARD rule (incoming)".to_string(),
        })?;
    if !output.status.success() {
        log::warn!(
            "Failed to add incoming FORWARD rule: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        log::info!("Added FORWARD rule: -i {} -j ACCEPT", bridge_name);
    }
    let output = Command::new("iptables")
        .args(["-I", "FORWARD", "1", "-o", bridge_name, "-j", "ACCEPT"])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to add FORWARD rule (outgoing)".to_string(),
        })?;
    if !output.status.success() {
        log::warn!(
            "Failed to add outgoing FORWARD rule: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        log::info!("Added FORWARD rule: -o {} -j ACCEPT", bridge_name);
    }
    log::info!(
        "NAT setup completed for {} (subnet: {})",
        bridge_name,
        subnet
    );
    let verify = Command::new("iptables")
        .args(["-t", "nat", "-L", "POSTROUTING", "-n"])
        .output()?;

    let output_str = String::from_utf8_lossy(&verify.stdout);
    if output_str.contains("MASQUERADE") && output_str.contains(subnet) {
        log::info!("MASQUERADE rule verified in iptables");
    } else {
        log::error!("MASQUERADE rule NOT found in iptables!");
        log::error!("Current POSTROUTING rules:\n{}", output_str);
        ContainerError::Network {
            message: format!("MASQUERADE rule verification failed"),
        };
    }
    let output = Command::new("iptables")
        .args([
            "-t",
            "nat",
            "-A",
            "POSTROUTING",
            "-s",
            "127.0.0.1",
            "-d",
            subnet,
            "-j",
            "MASQUERADE",
        ])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to add localhost MASQUERADE".to_string(),
        })?;
    if !output.status.success() {
        log::warn!(
            "Failed to add localhost MASQUERADE: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        log::info!("Added localhost MASQUERADE for hairpin NAT");
    }

    let _ = Command::new("iptables")
        .args(["-A", "FORWARD", "-i", bridge_name, "-j", "ACCEPT"])
        .output();
    let _ = Command::new("iptables")
        .args(["-A", "FORWARD", "-o", bridge_name, "-j", "ACCEPT"])
        .output();
    log::info!("Setup NAT for {}", bridge_name);
    Ok(())
}
pub fn cleanup_nat(bridge_name: &str) -> ContainerResult<()> {
    let _ = Command::new("iptables")
        .args(["-D", "FORWARD", "-i", bridge_name, "-j", "ACCEPT"])
        .output();
    let _ = Command::new("iptables")
        .args(["-D", "FORWARD", "-o", bridge_name, "-j", "ACCEPT"])
        .output();
    Ok(())
}
pub fn add_port_forward(
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
        .args([
            "-t",
            "nat",
            "-I",
            "PREROUTING",
            "1",
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
            message: "Failed to add PREROUTING DNAT".to_string(),
        })?;
    if !output.status.success() {
        ContainerError::Network {
            message: format!(
                "Failed to add port forward: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }
    log::info!(
        "Added PREROUTING DNAT: {}:{} -> {}:{}",
        host_port,
        proto,
        container_ip,
        container_port
    );
    let output = Command::new("iptables")
        .args([
            "-t",
            "nat",
            "-I",
            "OUTPUT",
            "1",
            "-p",
            proto,
            "-d",
            "127.0.0.1",
            "--dport",
            &host_port.to_string(),
            "-j",
            "DNAT",
            "--to-destination",
            &format!("{}:{}", container_ip, container_port),
        ])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to add OUTPUT DNAT".to_string(),
        })?;
    if !output.status.success() {
        log::warn!(
            "Failed to add OUTPUT DNAT (localhost): {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        log::info!(
            "Added OUTPUT DNAT for localhost: {}:{} -> {}:{}",
            host_port,
            proto,
            container_ip,
            container_port
        );
    }
    log::info!(
        "Port forward: {}:{} -> {}:{}",
        host_port,
        proto,
        container_ip,
        container_port
    );

    let _ = Command::new("iptables")
        .args([
            "-I",
            "FORWARD",
            "1",
            "-p",
            proto,
            "-d",
            &container_ip.to_string(),
            "--dport",
            &host_port.to_string(),
            "-j",
            "ACCEPT",
        ])
        .output();
    log::info!(
        "Port forward setup complete: {}:{} -> {}:{}",
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
        .args([
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
        .args([
            "-t",
            "nat",
            "-D",
            "OUTPUT",
            "-p",
            proto,
            "-d",
            "127.0.0.1",
            "--dport",
            &host_port.to_string(),
            "-j",
            "DNAT",
            "--to-destination",
            &format!("{}:{}", container_ip, container_port),
        ])
        .output();
    let _ = Command::new("iptables")
        .args([
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
    log::info!("Removed port forward rules: {}:{}", host_port, proto);
    Ok(())
}
pub fn enable_localhost_routing(bridge_name: &str) -> ContainerResult<()> {
    let all_path = "/proc/sys/net/ipv4/conf/all/route_localnet";
    let bridge_path = format!("/proc/sys/net/ipv4/conf/{}/route_localnet", bridge_name);
    fs::write(&all_path, "1").expect("Failed to enable route_localnet for all");
    fs::write(&bridge_path, "1").map_err(|_| ContainerError::Network {
        message: format!("Failed to enable route_localnet for {}", bridge_name),
    })?;
    log::info!("Enabled route_localnet for localhost routing");
    Ok(())
}
