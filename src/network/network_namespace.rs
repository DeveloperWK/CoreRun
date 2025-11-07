use std::{fs, net::Ipv4Addr, process::Command};

use nix::sched::{CloneFlags, setns};

use crate::error::{ContainerError, ContainerResult, Context};

#[derive(Debug)]
pub struct NetworkNamespace {
    pid: i32,
}
impl NetworkNamespace {
    pub fn from_pid(pid: i32) -> ContainerResult<Self> {
        let ns = NetworkNamespace { pid };
        Ok(ns)
    }
    pub fn enter<F, T>(&self, callback: F) -> ContainerResult<T>
    where
        F: FnOnce() -> ContainerResult<T>,
    {
        let current_ns = fs::File::open("/proc/self/ns/net")
            .map_err(|_| ContainerError::Network {
                message: "Failed to open current network namespace".to_string(),
            })
            .context("Network namespace failed")?;
        let ns_path = format!("/proc/{}/ns/net", self.pid);
        let container_ns = fs::File::open(&ns_path)
            .map_err(|_| ContainerError::Network {
                message: format!("Failed to open namespace for PID {}", self.pid),
            })
            .context("Network namespace failed")?;
        setns(&container_ns, CloneFlags::CLONE_NEWNET)
            .map_err(|e| ContainerError::Network {
                message: format!("Failed to enter container namespace: {}", e),
            })
            .context("Network namespace failed")?;
        let result = callback();
        setns(&current_ns, CloneFlags::CLONE_NEWNET)
            .map_err(|_| ContainerError::Network {
                message: "Failed to return to original namespace".to_string(),
            })
            .context("Network namespace failed")?;
        result
    }
    pub fn setup_loopback(&self) -> ContainerResult<()> {
        self.enter(|| {
            let output = Command::new("ip")
                .args(["link", "set", "lo", "up"])
                .output()
                .map_err(|_| ContainerError::Network {
                    message: "Failed to execute ip command".to_string(),
                })?;
            if !output.status.success() {
                ContainerError::Network {
                    message: format!(
                        "Failed to setup loopback: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                };
            }
            Ok(())
        })
    }
    pub fn configure_interface(
        &self,
        interface: &str,
        ip: Ipv4Addr,
        subnet_prefix: u8,
    ) -> ContainerResult<()> {
        self.enter(|| {
            let ip_with_prefix = format!("{}/{}", ip, subnet_prefix);
            let output = Command::new("ip")
                .args(["addr", "add", &ip_with_prefix, "dev", interface])
                .output()
                .map_err(|_| ContainerError::Network {
                    message: "Failed to set IP address".to_string(),
                })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.contains("File exists") {
                    ContainerError::Network {
                        message: format!("Failed to configure interface: {}", stderr),
                    };
                }
            }
            let output = Command::new("ip")
                .args(["link", "set", interface, "up"])
                .output()
                .map_err(|_| ContainerError::Network {
                    message: "Failed to bring interface up".to_string(),
                })?;
            if !output.status.success() {
                ContainerError::Network {
                    message: format!(
                        "Failed to bring interface up: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                };
            }
            log::debug!("Interface {} configured with {}", interface, ip_with_prefix);
            Ok(())
        })
    }
    pub fn add_default_route(&self, interface: &str, gateway: Ipv4Addr) -> ContainerResult<()> {
        self.enter(|| {
            let output = Command::new("ip")
                .args([
                    "route",
                    "add",
                    "default",
                    "via",
                    &gateway.to_string(),
                    "dev",
                    interface,
                ])
                .output()
                .map_err(|_| ContainerError::Network {
                    message: "Failed to add default route".to_string(),
                })?;
            if !output.status.success() {
                ContainerError::Network {
                    message: format!(
                        "Failed to add default route: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                };
            }
            Ok(())
        })
    }
}
