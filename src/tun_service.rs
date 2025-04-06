use std::sync::Arc;

use anyhow::anyhow;
use injector::WeakInjected;
use injector::{Injectable, Injected};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, warn};
use tun::{AbstractDevice, DeviceReader, DeviceWriter};
use tun::configure;
use tun::create_as_async;

use crate::config_service::ConfigService;
use crate::iroh_service::{IrohService, PeerMessage};
use crate::mailbox::Mailbox;

#[derive(Injectable)]
pub struct TunService {
    pub packet_stream: Mailbox<Arc<[u8]>, 256>,
    iroh_service: WeakInjected<IrohService>,
    config_service: Injected<ConfigService>,
}

impl TunService {
    pub async fn run(self: Injected<Self>) -> anyhow::Result<()> {
        let mut config = configure();
        if let Some(address) = self.config_service.config().tun_address.clone() {
            config.address(address);
        }
        if let Some(netmask) = self.config_service.config().tun_netmask.clone() {
            config.netmask(netmask);
        }
        if let Some(name) = self.config_service.config().tun_name.clone() {
            config.tun_name(name);
        } else {
            config.tun_name("iroh-vpn");
        }
        #[cfg(target_os = "linux")]
        config.platform_config(|config| {
            // requiring root privilege to acquire complete functions
            config.ensure_root_privileges(true);
        });
        config.up();

        let device = create_as_async(&config)?;

        info!("TUN created with index {}", device.tun_index()?);
        info!("TUN IP address: {}", device.address()?);
        let mtu = device.mtu()?;

        let (device_writer, device_reader) = device.split()?;

        tokio::select!{
            result = self.clone().writer_loop(device_writer) => result,
            result = self.reader_loop(mtu, device_reader) => result,
        }?;

        Ok(())
    }

    async fn writer_loop(self: Injected<Self>, mut device_writer: DeviceWriter) -> anyhow::Result<()> {
        let mut receiver = self.packet_stream.broadcast.subscribe();
        loop {
            match receiver.recv().await {
                Ok(packet) => {
                    device_writer.write_all(&packet).await?;
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
        Ok(())
    }

    async fn reader_loop(self: Injected<Self>, mtu: u16, mut device_reader: DeviceReader) -> anyhow::Result<()> {
        let packet_broadcast = self.iroh_service.upgrade().ok_or(anyhow!("TUN service could not get iroh service"))?.packet_broadcast.broadcast.clone();
        let mut buffer = vec![0u8; mtu as usize];
        loop {
            match device_reader.read(&mut buffer).await {
                Ok(n) => {
                    let _ = packet_broadcast.send(PeerMessage::Packet(Arc::from(&buffer[..n])));
                }
                Err(err) => {
                    warn!("{}", err);
                }
            }
        }
    }
}
