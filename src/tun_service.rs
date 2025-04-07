use std::future::poll_fn;
use std::io::Write;
use std::sync::Arc;
use std::task::Poll;

use anyhow::anyhow;
use futures::Stream;
use injector::WeakInjected;
use injector::{Injectable, Injected};
use tokio::io::{AsyncRead, ReadBuf};
use tracing::{debug, info};
use tun::AbstractDevice;
use tun::create_as_async;
use tun::{Device, configure};

use crate::config_service::ConfigService;
use crate::iroh_service::{IrohService, PeerMessage};
use crate::mailbox::Mailbox;
use crate::receiver_stream::ReceiverStream;

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

        let mut device = Box::pin(create_as_async(&config)?);

        info!("TUN created with index {}", device.tun_index()?);
        info!("TUN IP address: {}", device.address()?);
        let mtu = device.mtu()?;

        let mut buf_memory = vec![0u8; mtu as usize];
        let mut buf = ReadBuf::new(&mut buf_memory);
        let receiver = self.packet_stream.broadcast.subscribe();
        let mut recv_stream = Box::pin(ReceiverStream::new(receiver));
        let packet_broadcast = self
            .iroh_service
            .upgrade()
            .ok_or(anyhow!("TUN service could not get iroh service"))?
            .packet_broadcast
            .broadcast
            .clone();

        let () = poll_fn(|cx| {
            if let Poll::Ready(_) = device.as_mut().poll_read(cx, &mut buf) {
                let data: Arc<[u8]> = Arc::from(buf.filled());
                debug!("Sending {} bytes from TUN", data.len());
                let _ = packet_broadcast.send(PeerMessage::Packet(data));
                buf.clear();
                cx.waker().wake_by_ref();
            }
            if let Poll::Ready(Some(data)) = recv_stream.as_mut().poll_next(cx) {
                debug!("Sending {} bytes to TUN", data.len());
                let _ = (&mut device as &mut Device).write_all(&data);
                cx.waker().wake_by_ref();
            }
            Poll::Pending
        })
        .await;

        Ok(())
    }
}
