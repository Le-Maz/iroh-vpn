use std::sync::Arc;

use injector::Injected;
use injector_macro::Injectable;
use tun::configure;
use tun::create_as_async;

use crate::mailbox::Mailbox;

#[derive(Debug)]
pub enum TunServiceMessage {
    Packet(Arc<[u8]>),
}

#[derive(Injectable)]
pub struct TunService {
    pub mailbox: Mailbox<TunServiceMessage>,
}

impl TunService {
    pub async fn run(self: Injected<Self>) -> anyhow::Result<()> {
        let mut config = configure();
        config
            .address((10, 0, 0, 9))
            .netmask((255, 255, 255, 0))
            .destination((10, 0, 0, 1))
            .up();

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
