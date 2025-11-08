use crate::{
    error::{ContainerError, ContainerResult},
    network::{
        ContainerNetwork, NetworkMode, NetworkNamespace, PortMapping, bridge::Bridge, iptables,
        veth,
    },
};

use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    process::Command,
    sync::{Arc, Mutex},
};

pub struct NetworkManager {
    networks: Arc<Mutex<HashMap<String, NetworkConfig>>>,
    container_networks: Arc<Mutex<HashMap<String, ContainerNetwork>>>,
}

struct NetworkConfig {
    #[allow(dead_code)]
    name: String,
    bridge: Bridge,
    subnet: ipnetwork::Ipv4Network,
    gateway: Ipv4Addr,
    allocator: IpAllocator,
}
impl NetworkManager {
    pub fn new() -> ContainerResult<Self> {
        Self::check_other_runtimes();
        let manager = Self {
            networks: Arc::new(Mutex::new(HashMap::new())),
            container_networks: Arc::new(Mutex::new(HashMap::new())),
        };

        manager.create_network("bridge", "172.18.0.0/16")?;

        Ok(manager)
    }
    fn check_other_runtimes() {
        let docker_running = Command::new("docker")
            .arg("info")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let podman_running = Command::new("podman")
            .arg("info")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if docker_running {
            log::info!("ℹ️ Docker detected - using separate subnet (172.18.0.0/16)");
        }
        if podman_running {
            log::info!("ℹ️ Podman detected - using separate subnet (172.18.0.0/16)");
        }

        if docker_running && podman_running {
            log::info!("ℹ️ Both runtimes can coexist without conflicts");
        }
    }
    pub fn create_network(&self, name: &str, subnet: &str) -> ContainerResult<()> {
        let subnet: ipnetwork::Ipv4Network =
            subnet.parse().map_err(|_| ContainerError::Network {
                message: "Invalid subnet".to_string(),
            })?;
        let bridge_name = if name == "bridge" {
            "corerun0".to_string()
        } else {
            format!("cr-{}", &name[..std::cmp::min(8, name.len())])
        };
        let bridge = Bridge::new(&bridge_name)?;
        bridge.create()?;
        let gateway = subnet.iter().nth(1).unwrap();
        bridge.set_ip(gateway, subnet.prefix())?;
        bridge.up()?;
        iptables::enable_localhost_routing(&bridge_name)?;
        iptables::setup_nat(&bridge_name, &subnet.to_string())?;
        let config = NetworkConfig {
            name: name.to_string(),
            bridge,
            subnet,
            gateway,
            allocator: IpAllocator::new(subnet)?,
        };
        self.networks
            .lock()
            .unwrap()
            .insert(name.to_string(), config);

        log::info!("Created network '{}' with subnet {}", name, subnet);
        Ok(())
    }
    pub fn setup_container_network(
        &self,
        container_id: &str,
        pid: i32,
        mode: NetworkMode,
        ports: Vec<PortMapping>,
    ) -> ContainerResult<ContainerNetwork> {
        match mode {
            NetworkMode::Bridge { network_name } => {
                self.setup_bridge_network(container_id, pid, &network_name, ports)
            }
            NetworkMode::Host => self.setup_host_network(container_id),
            NetworkMode::None => self.setup_none_network(container_id, pid),
            NetworkMode::Container {
                container_id: ref target_id,
            } => self.setup_container_network_shared(container_id, target_id),
        }
    }
    fn setup_bridge_network(
        &self,
        container_id: &str,
        pid: i32,
        network_name: &str,
        ports: Vec<PortMapping>,
    ) -> ContainerResult<ContainerNetwork> {
        let mut networks = self.networks.lock().unwrap();
        let network = networks.get_mut(network_name).unwrap();
        log::info!(
            "Allocator state before allocation: {:?} IPs allocated",
            network.allocator.allocated.len()
        );
        let container_ip = network.allocator.allocate()?;

        log::info!(
            "Allocated IP: {} for container {}",
            container_ip,
            &container_id[..12]
        );
        log::info!(
            "Allocator state after allocation: {:?} IPs allocated",
            network.allocator.allocated.len()
        );
        let veth_host = format!("veth{}", &container_id[10..17]);
        let veth_container = format!("vethc{}", &container_id[10..17]);
        log::info!("Creating veth pair: {} <-> {}", veth_host, veth_container);
        match veth::create_veth_pair(&veth_host, &veth_container) {
            Ok(_) => log::info!("✅ Veth pair created"),
            Err(e) => {
                log::error!("❌ Failed to create veth pair: {}", e);
                return Err(e);
            }
        }

        log::info!("Attaching {} to bridge", veth_host);
        network.bridge.attach_interface(&veth_host)?;
        log::info!("Moving {} to namespace PID {}", veth_container, pid);
        veth::move_to_namespace(&veth_container, pid)?;

        let ns = NetworkNamespace::from_pid(pid)?;

        log::info!("Setting up loopback in container");
        ns.setup_loopback()?;
        log::info!("Renaming {} to eth0", veth_container);
        let _ = ns.enter(|| {
            let output = Command::new("ip")
                .args(["link", "set", &veth_container, "name", "eth0"])
                .output()
                .map_err(|_| ContainerError::Network {
                    message: "Failed to rename interface to eth0".to_string(),
                })?;
            if !output.status.success() {
                ContainerError::Network {
                    message: format!(
                        "Failed to rename interface: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                };
            }
            Ok(())
        });
        log::info!("Configuring eth0 with IP {}", container_ip);
        ns.configure_interface("eth0", container_ip, network.subnet.prefix())?;
        log::info!("Adding default route via {}", network.gateway);
        ns.add_default_route("eth0", network.gateway)?;
        for port in &ports {
            iptables::add_port_forward(
                port.host_port,
                container_ip,
                port.container_port,
                port.protocol,
            )?;
        }
        let container_network = ContainerNetwork {
            mode: NetworkMode::Bridge {
                network_name: network_name.to_string(),
            },
            ip_address: Some(container_ip),
            gateway: Some(network.gateway),
            veth_host: Some(veth_host),
            veth_container: Some(veth_container),
            ports,
        };
        self.container_networks
            .lock()
            .unwrap()
            .insert(container_id.to_string(), container_network.clone());
        log::info!(
            "Container {} network: IP={}, Gateway={}",
            &container_id[..12],
            container_ip,
            network.gateway
        );

        Ok(container_network)
    }
    fn setup_host_network(&self, container_id: &str) -> ContainerResult<ContainerNetwork> {
        let container_network = ContainerNetwork {
            mode: NetworkMode::Host,
            gateway: None,
            ip_address: None,
            ports: vec![],
            veth_container: None,
            veth_host: None,
        };
        self.container_networks
            .lock()
            .unwrap()
            .insert(container_id.to_string(), container_network.clone());
        log::info!("Container {} using host networking", &container_id[..12]);
        Ok(container_network)
    }
    fn setup_none_network(
        &self,
        container_id: &str,
        pid: i32,
    ) -> ContainerResult<ContainerNetwork> {
        let ns = NetworkNamespace::from_pid(pid)?;
        ns.setup_loopback()?;

        let container_network = ContainerNetwork {
            mode: NetworkMode::None,
            gateway: None,
            ip_address: None,
            ports: vec![],
            veth_container: None,
            veth_host: None,
        };
        self.container_networks
            .lock()
            .unwrap()
            .insert(container_id.to_string(), container_network.clone());
        log::info!(
            "Container {} using no networking (isolated)",
            &container_id[..12]
        );
        Ok(container_network)
    }
    fn setup_container_network_shared(
        &self,
        container_id: &str,
        target_container_id: &str,
    ) -> ContainerResult<ContainerNetwork> {
        let networks = self.container_networks.lock().unwrap();
        let target_network = networks.get(target_container_id).unwrap();
        let container_network = ContainerNetwork {
            mode: NetworkMode::Container {
                container_id: target_container_id.to_string(),
            },
            gateway: target_network.gateway,
            ip_address: target_network.ip_address,
            veth_container: None,
            veth_host: None,
            ports: vec![],
        };
        drop(networks);
        self.container_networks
            .lock()
            .unwrap()
            .insert(container_id.to_string(), container_network.clone());

        log::info!(
            "Container {} sharing network with {}",
            &container_id[..12],
            &target_container_id[..12]
        );

        Ok(container_network)
    }
    pub fn cleanup_container_network(&self, container_id: &str) -> ContainerResult<()> {
        let mut container_networks = self.container_networks.lock().unwrap();

        if let Some(network) = container_networks.remove(container_id) {
            match network.mode {
                NetworkMode::Bridge { network_name } => {
                    for port in &network.ports {
                        if let Some(ip) = network.ip_address {
                            let _ = iptables::remove_port_forward(
                                port.host_port,
                                ip,
                                port.container_port,
                                port.protocol,
                            );
                        }
                    }
                    if let Some(ip) = network.ip_address {
                        let mut networks = self.networks.lock().unwrap();
                        if let Some(net) = networks.get_mut(&network_name) {
                            net.allocator.release(ip);
                        }
                    }
                    if let Some(veth_host) = &network.veth_host {
                        let _ = veth::delete_veth(veth_host).map_err(|_| ContainerError::Network {
                            message: "failed to delete veth ".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn _delete_network(&self, name: &str) -> ContainerResult<()> {
        if name == "bridge" {
            ContainerError::Network {
                message: "Cannot delete default bridge network".to_string(),
            };
        }
        let mut networks = self.networks.lock().unwrap();
        if let Some(network) = networks.remove(name) {
            network.bridge.delete()?;
            iptables::cleanup_nat(&network.bridge.name)?;
            log::info!("Deleted network '{}'", name);
        }
        Ok(())
    }
}
#[derive(Clone)]
struct IpAllocator {
    subnet: ipnetwork::Ipv4Network,
    allocated: HashSet<Ipv4Addr>,
}
impl IpAllocator {
    fn new(subnet: ipnetwork::Ipv4Network) -> ContainerResult<Self> {
        Ok(Self {
            subnet,
            allocated: HashSet::new(),
        })
    }
    fn allocate(&mut self) -> ContainerResult<Ipv4Addr> {
        self.scan_existing_ips()?;
        for ip in self.subnet.iter().skip(2) {
            if !self.allocated.contains(&ip) {
                self.allocated.insert(ip);
                log::debug!("Allocated IP: {}", ip);
                return Ok(ip);
            }
        }
        Err(ContainerError::Network {
            message: "No available IPs in subnet".to_string(),
        })
    }
    fn release(&mut self, ip: Ipv4Addr) -> () {
        self.allocated.remove(&ip);
        log::debug!("Released IP: {}", ip);
    }

    fn scan_existing_ips(&mut self) -> ContainerResult<()> {
        log::info!(
            "🔍 Scanning for active container IPs in subnet {}...",
            self.subnet
        );
        // Directly ping first 10 possible container IPs
        for ip in self.subnet.iter().skip(2).take(20) {
            let output = Command::new("ping")
                .args(["-c", "1", "-W", "1", &ip.to_string()])
                .output();

            if let Ok(result) = output {
                if result.status.success() {
                    self.allocated.insert(ip);
                    log::info!("✅ Found active container IP: {}", ip);
                }
            }
        }

        log::info!("Scan complete: {} active IPs found", self.allocated.len());
        Ok(())
    }
}
