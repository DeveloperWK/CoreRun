use std::{fs, process::Command};

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
                message: format!("Failed to open current network namespace"),
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
            .map_err(|e| ContainerError::Network {
                message: format!("Failed to return to original namespace"),
            })
            .context("Network namespace failed")?;
        result
    }
    pub fn setup_loopback(&self) -> ContainerResult<()> {
        self.enter(|| {
            let output = Command::new("ip")
                .args(&["link", "set", "lo", "up"])
                .output()
                .map_err(|_| ContainerError::Network {
                    message: format!("Failed to execute ip command"),
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
}
