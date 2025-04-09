use std::sync::LazyLock;

use clap::Parser;
use iroh::NodeId;

/// Peer-to-peer VPN using [Iroh](https://www.iroh.computer/)
#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long)]
    pub tun_address: Option<String>,
    #[arg(long)]
    pub tun_netmask: Option<String>,
    #[arg(long)]
    pub tun_name: Option<String>,
    #[arg(long)]
    pub iroh_sk_path: Option<String>,
    #[arg(long)]
    pub iroh_peer_ids: Option<Vec<NodeId>>,
}

pub static CONFIG: LazyLock<Args> = LazyLock::new(|| Args::parse());
