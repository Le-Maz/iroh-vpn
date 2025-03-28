use std::sync::Arc;

use injector::Injected;
use injector_macro::Injectable;
use tun::configure;
use tun::create_as_async;

use crate::config_service::ConfigService;
use crate::mailbox::Mailbox;

#[derive(Debug)]
pub enum TunServiceMessage {
    Packet(Arc<[u8]>),
}

#[derive(Injectable)]
pub struct TunService {
    pub mailbox: Mailbox<TunServiceMessage>,
    config_service: Injected<ConfigService>,
}

impl TunService {
    pub async fn run(self: Injected<Self>) -> anyhow::Result<()> {
        let mut config = configure();
        if let Some(address) = (&self.config_service).config().tun.address.clone() {
            config.address(address);
        }
        if let Some(netmask) = (&self.config_service).config().tun.netmask.clone() {
            config.netmask(netmask);
        }
        if let Some(name) = (&self.config_service).config().tun.name.clone() {
            config.tun_name(name);
        }
        #[cfg(target_os = "linux")]
        config.platform_config(|config| {
            // requiring root privilege to acquire complete functions
            config.ensure_root_privileges(true);
        });
        config.up();

        let device = create_as_async(&config)?;
        let mut receiver = self.mailbox.lock_receiver().await;
        while let Some(message) = receiver.recv().await {
            match message {
                TunServiceMessage::Packet(body) => {
                    device.send(&body).await?;
                }
            }
        }
        Ok(())
    }
}
