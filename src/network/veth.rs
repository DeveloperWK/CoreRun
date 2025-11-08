use std::process::Command;

use crate::error::{ContainerError, ContainerResult};

pub fn create_veth_pair(veth_host: &str, veth_container: &str) -> ContainerResult<()> {
    let output = Command::new("ip")
        .args([
            "link",
            "add",
            veth_host,
            "type",
            "veth",
            "peer",
            "name",
            veth_container,
        ])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to create veth pair".to_string(),
        })?;
    if !output.status.success() {
        ContainerError::Network {
            message: format!(
                "Failed to create veth pair: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }
    log::debug!("Created veth pair: {} <-> {}", veth_host, veth_container);
    Ok(())
}
pub fn move_to_namespace(interface: &str, pid: i32) -> ContainerResult<()> {
    let output = Command::new("ip")
        .args(["link", "set", interface, "netns", &pid.to_string()])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to move interface to namespace".to_string(),
        })?;
    if !output.status.success() {
        ContainerError::Network {
            message: format!(
                "Failed to move interface to namespace: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }
    log::debug!("Moved {} to namespace of PID {}", interface, pid);
    Ok(())
}
pub fn delete_veth(interface: &str) -> ContainerResult<()> {
    let output = Command::new("ip")
        .args(["link", "delete", interface])
        .output()
        .map_err(|_| ContainerError::Network {
            message: "Failed to delete veth".to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("Cannot find device") {
            ContainerError::Network {
                message: format!("Failed to delete veth: {}", stderr),
            };
        }
    }

    Ok(())
}
