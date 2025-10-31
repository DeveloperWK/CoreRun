pub mod bridge;
pub mod iptables;
pub mod net_manager;

pub mod network_namespace;
pub mod veth;
use std::net::Ipv4Addr;

pub use net_manager::*;

pub use network_namespace::*;

use crate::error::{ContainerError, ContainerResult};
#[derive(Debug, Clone)]
pub enum NetworkMode {
    Bridge { network_name: String },
    Host,
    None,
    Container { container_id: String },
}
// #[derive(Debug, Clone)]
// pub struct NetworkConfig {
//     pub mode: NetworkMode,
//     pub ports: Vec<PortMapping>,
//     pub dns_server: Vec<Ipv4Addr>,
//     pub hostname: Option<String>,
// }
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Protocol,
}
impl PortMapping {
    pub fn parse(s: &str) -> ContainerResult<Self> {
        let parts: Vec<&str> = s.split('/').collect();
        let port_part = parts[0];
        let protocol = if parts.len() > 1 {
            match parts[1].to_lowercase().as_str() {
                "tcp" => Protocol::TCP,
                "upd" => Protocol::UDP,
                _ => {
                    return Err(ContainerError::Network {
                        message: format!("Invalid protocol: {}", parts[1]),
                    });
                }
            }
        } else {
            Protocol::TCP
        };
        let port_parts: Vec<&str> = port_part.split(':').collect();
        if port_parts.len() != 2 {
            return Err(ContainerError::Network {
                message: format!("Port mapping must be in format HOST:CONTAINER"),
            });
        }
        let host_port = port_parts[0].parse().map_err(|_| ContainerError::Network {
            message: format!("Invalid host port: {}", port_parts[0]),
        })?;
        let container_port = port_parts[1].parse().map_err(|_| ContainerError::Network {
            message: format!("Invalid container port: {}", port_parts[1]),
        })?;
        Ok(PortMapping {
            host_port,
            container_port,
            protocol,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    UDP,
    TCP,
}
#[derive(Debug, Clone)]
pub struct ContainerNetwork {
    pub mode: NetworkMode,
    pub ip_address: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub veth_host: Option<String>,
    pub veth_container: Option<String>,
    pub ports: Vec<PortMapping>,
}
