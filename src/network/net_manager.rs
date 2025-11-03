use crate::{
    error::{ContainerError, ContainerResult},
    network::{
        ContainerNetwork, NetworkMode, NetworkNamespace, PortMapping,
        bridge::{self, Bridge},
        iptables, veth,
    },
};

use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};

pub struct NetworkManager {
    networks: Arc<Mutex<HashMap<String, NetworkConfig>>>,
    container_networks: Arc<Mutex<HashMap<String, ContainerNetwork>>>,
}

#[derive(Clone)]
struct NetworkConfig {
    name: String,
    bridge: Bridge,
    subnet: ipnetwork::Ipv4Network,
    gateway: Ipv4Addr,
    allocator: IpAllocator,
}
impl NetworkManager {
    pub fn new() -> ContainerResult<Self> {
        let manager = Self {
            networks: Arc::new(Mutex::new(HashMap::new())),
            container_networks: Arc::new(Mutex::new(HashMap::new())),
        };
        manager.create_network("bridge", "172.17.0.0/16")?;

        Ok(manager)
    }
    pub fn create_network(&self, name: &str, subnet: &str) -> ContainerResult<()> {
        let subnet: ipnetwork::Ipv4Network =
            subnet.parse().map_err(|_| ContainerError::Network {
                message: format!("Invalid subnet"),
            })?;
        let bridge_name = if name == "bridge" {
            "corerun0".to_string()
        } else {
            format!("br-{}", &name[..std::cmp::min(8, name.len())])
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
        let container_ip = network.allocator.allocate()?;
        let veth_host = format!("veth{}", &container_id[..7]);
        let veth_container = "eth0".to_string();
        veth::create_veth_pair(&veth_host, &veth_container)?;
        network.bridge.attach_interface(&veth_host)?;
        veth::move_to_namespace(&veth_container, pid)?;
        let ns = NetworkNamespace::from_pid(pid)?;
        ns.setup_loopback()?;
        ns.configure_interface(&veth_container, container_ip, network.subnet.prefix())?;
        ns.add_default_route(&veth_container, network.gateway)?;
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
        let target_network = networks.get(target_container_id).clone().unwrap();
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
                message: format!("Cannot delete default bridge network"),
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
        for ip in self.subnet.iter().skip(2) {
            if !self.allocated.contains(&ip) {
                self.allocated.insert(ip);
                return Ok(ip);
            }
        }
        Err(ContainerError::Network {
            message: format!("No available IPs in subnet"),
        })
    }
    fn release(&mut self, ip: Ipv4Addr) -> () {
        self.allocated.remove(&ip);
    }
}
