use std::{
    net::{IpAddr, Ipv4Addr},
    process::Command,
};

use crate::{
    error::{ContainerError, ContainerResult},
    network::ContainerNetwork,
};
#[derive(Clone)]
pub struct Bridge {
    pub name: String,
}
impl Bridge {
    pub fn new(name: &str) -> ContainerResult<Self> {
        Ok(Self {
            name: name.to_string(),
        })
    }
    pub fn create(&self) -> ContainerResult<()> {
        if self.exists()? {
            log::info!("Bridge {} already exists", self.name);
            return Ok(());
        }
        let output = Command::new("ip")
            .args(&["link", "add", "name", &self.name, "type", "bridge"])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to create bridge"),
            })?;
        if !output.status.success() {
            ContainerError::Network {
                message: format!(
                    "Failed to create bridge: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            };
        }
        log::info!("Created bridge {}", self.name);
        Ok(())
    }
    pub fn delete(&self) -> ContainerResult<()> {
        let output = Command::new("ip")
            .args(&["link", "delete", &self.name])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to delete bridge"),
            })?;
        if !output.status.success() {
            ContainerError::Network {
                message: format!(
                    "Failed to delete bridge: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            };
        }
        Ok(())
    }
    pub fn exists(&self) -> ContainerResult<bool> {
        let output = Command::new("ip")
            .args(&["link", "show", &self.name])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to check bridge existence"),
            })?;
        Ok(output.status.success())
    }
    pub fn set_ip(&self, ip: Ipv4Addr, prefix: u8) -> ContainerResult<()> {
        let ip_with_prefix = format!("{}/{}", ip, prefix);
        let output = Command::new("ip")
            .args(&["addr", "add", &ip_with_prefix, "dev", &self.name])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to set bridge IP"),
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("File exists") {
                ContainerError::Network {
                    message: format!("Failed to set bridge IP: {}", stderr),
                };
            }
        }
        Ok(())
    }
    pub fn up(&self) -> ContainerResult<()> {
        let output = Command::new("ip")
            .args(&["link", "set", &self.name, "up"])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to bring bridge up"),
            })?;
        if !output.status.success() {
            ContainerError::Network {
                message: format!(
                    "Failed to bring bridge up: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            };
        }
        Ok(())
    }
    pub fn attach_interface(&self, interface: &str) -> ContainerResult<()> {
        let output = Command::new("ip")
            .args(&["link", "set", interface, "master", &self.name])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to attach interface to bridge"),
            })?;
        if !output.status.success() {
            ContainerError::Network {
                message: format!(
                    "Failed to attach interface to bridge: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            };
        }
        let output = Command::new("ip")
            .args(&["link", "set", interface, "up"])
            .output()
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to bring interface up"),
            })?;
        if !output.status.success() {
            ContainerError::Network {
                message: format!(
                    "Failed to bring interface up: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            };
        }
        Ok(())
    }
}
