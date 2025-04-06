use std::sync::{Arc, OnceLock};

use clap::Parser;
use injector::Injectable;

/// Peer-to-peer VPN using [Iroh](https://www.iroh.computer/)
#[derive(Debug, Parser)]
pub struct Config {
    pub tun_address: Option<String>,
    pub tun_netmask: Option<String>,
    pub tun_name: Option<String>,
    pub iroh_sk_path: Option<String>,
}

#[derive(Injectable)]
pub struct ConfigService {
    config: OnceLock<Arc<Config>>,
}

impl ConfigService {
    pub fn config(&self) -> Arc<Config> {
        self.config
            .get_or_init(|| Arc::new(Self::load_config()))
            .clone()
    }
    fn load_config() -> Config {
        Config::parse()
    }
}
