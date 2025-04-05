use std::sync::{Arc, OnceLock};

use injector::Injectable;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct TunConfig {
    pub address: Option<String>,
    pub netmask: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tun: TunConfig,
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
        serde_json::from_str("{}").unwrap()
    }
}
