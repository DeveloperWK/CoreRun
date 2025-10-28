use super::*;
use crate::error::{ContainerError, ContainerResult};
use std::net::Ipv4Addr;

impl NetworkConfig {
    pub fn parse(network_str: &str) -> ContainerResult<Self> {
        match network_str {
            "bridge" => Ok(NetworkConfig {
                mode: NetworkMode::Bridge,
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
            "host" => Ok(NetworkConfig {
                mode: NetworkMode::Host,
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
            "none" => Ok(NetworkConfig {
                mode: NetworkMode::None,
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
            network if network.starts_with("container:") => {
                let contsiner_id = network.trim_start_matches("container:").to_string();
                Ok(NetworkConfig {
                    mode: NetworkMode::Container(contsiner_id),
                    ports: Vec::new(),
                    dns_server: Vec::new(),
                    hostname: None,
                })
            }
            custom => Ok(NetworkConfig {
                mode: NetworkMode::Custom(custom.to_string()),
                ports: Vec::new(),
                dns_server: Vec::new(),
                hostname: None,
            }),
        }
    }
    pub fn add_port_mapping(&mut self, port_str: &str) -> ContainerResult<()> {
        let mapping = PortMapping::parse(&port_str)?;
        self.ports.push(mapping);
        Ok(())
    }
}
impl PortMapping {
    pub fn parse(port_str: &str) -> ContainerResult<Self> {
        let parts: Vec<&str> = port_str.split('/').collect();
        let (ports, protocol) = match parts.len() {
            1 => (parts[0], "tcp"),
            2 => (parts[0], parts[1]),
            _ => {
                return Err(ContainerError::Network {
                    message: format!("Invalid port format: {}", port_str),
                });
            }
        };
        let protocol = match protocol.to_lowercase().as_str() {
            "tcp" => Protocol::TCP,
            "udp" => Protocol::UDP,
            _ => {
                return Err(ContainerError::Network {
                    message: format!("Invalid protocol: {}", protocol),
                });
            }
        };
        let port_parts: Vec<&str> = ports.split(":").collect();
        match port_parts.len() {
            1 => {
                let container_port =
                    port_parts[0]
                        .parse::<u16>()
                        .map_err(|e| ContainerError::Network {
                            message: format!("Invalid Container port: {}", port_parts[0]),
                        })?;
                Ok(PortMapping {
                    host_port: 0,
                    container_port,
                    protocol,
                })
            }
            2 => {
                let host_port =
                    port_parts[0]
                        .parse::<u16>()
                        .map_err(|e| ContainerError::Network {
                            message: format!("Invalid Host port: {}", port_parts[0]),
                        })?;
                let container_port =
                    port_parts[1]
                        .parse::<u16>()
                        .map_err(|e| ContainerError::Network {
                            message: format!("Invalid Container port: {}", port_parts[1]),
                        })?;
                Ok(PortMapping {
                    host_port,
                    container_port,
                    protocol,
                })
            }
            _ => Err(ContainerError::Network {
                message: format!("Invalid port mapping: {}", ports),
            }),
        }
    }
}
