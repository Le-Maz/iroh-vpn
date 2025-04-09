use std::sync::LazyLock;

use clap::Parser;

/// Peer-to-peer VPN using [Iroh](https://www.iroh.computer/)
#[derive(Debug, Parser)]
pub struct Config {
    pub tun_address: Option<String>,
    pub tun_netmask: Option<String>,
    pub tun_name: Option<String>,
    pub iroh_sk_path: Option<String>,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::parse());
