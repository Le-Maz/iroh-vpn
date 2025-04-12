use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::{Arc, LazyLock, RwLock},
};

use clap::Parser;
use iroh::{NodeId, SecretKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

/// Peer-to-peer VPN using [Iroh](https://www.iroh.computer/)
#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, short)]
    pub config_file: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerConfig {
    pub address: IpAddr,
    #[serde(with = "serde_str")]
    pub node_id: NodeId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IrohConfig {
    #[serde(default = "generate_sk")]
    #[serde(with = "serde_str")]
    pub sk: SecretKey,
    #[serde(default)]
    pub peers: Vec<PeerConfig>,
}

impl Default for IrohConfig {
    fn default() -> Self {
        Self {
            sk: generate_sk(),
            peers: Default::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TunConfig {
    pub address: Option<Ipv4Addr>,
    pub netmask: Option<Ipv4Addr>,
    pub name: Option<String>,
    pub mtu: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub iroh: IrohConfig,
    #[serde(default)]
    pub tun: TunConfig,
}

pub static ARGS: LazyLock<Args> = LazyLock::new(Args::parse);

static CONFIG: LazyLock<RwLock<Arc<Config>>> = LazyLock::new(|| {
    let path = ARGS
        .config_file
        .clone()
        .or_else(|| Some(dirs::config_dir()?.join("config.json")))
        .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let config = serde_json::from_str(&text).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    config
});

pub fn get_config() -> Arc<Config> {
    return CONFIG.read().unwrap().clone();
}

pub static TUN_CONFIG: LazyLock<tun::Configuration> = LazyLock::new(|| {
    let config = get_config();
    let mut tun_config = tun::configure();
    if let Some(address) = config.tun.address.clone() {
        tun_config.address(address);
    }
    if let Some(netmask) = config.tun.netmask.clone() {
        tun_config.netmask(netmask);
    }
    if let Some(mtu) = config.tun.mtu {
        tun_config.mtu(mtu);
    }
    if let Some(name) = config.tun.name.clone() {
        tun_config.tun_name(name);
    } else {
        tun_config.tun_name("iroh-vpn");
    }
    #[cfg(target_os = "linux")]
    tun_config.platform_config(|tun_config| {
        // requiring root privilege to acquire complete functions
        tun_config.ensure_root_privileges(true);
    });
    tun_config.up();
    tun_config
});

pub static WHITELIST: LazyLock<HashSet<NodeId>> = LazyLock::new(|| {
    get_config()
        .iroh
        .peers
        .iter()
        .map(|peer| peer.node_id)
        .collect()
});

fn generate_sk() -> SecretKey {
    SecretKey::generate(&mut OsRng)
}
