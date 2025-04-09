use std::future::poll_fn;
use std::io::Write;
use std::sync::Arc;
use std::task::Poll;

use futures::Stream;
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info};
use tun::AbstractDevice;
use tun::create_as_async;
use tun::{Device, configure};

use crate::config::CONFIG;
use crate::peers::PeersMessage;

pub enum TunMessage {
    Packet(Arc<[u8]>),
}

pub async fn run_tun(
    tun_recv: mpsc::Receiver<TunMessage>,
    peers_send: mpsc::Sender<PeersMessage>,
) -> anyhow::Result<()> {
    let mut config = configure();
    if let Some(address) = CONFIG.tun_address.clone() {
        config.address(address);
    }
    if let Some(netmask) = CONFIG.tun_netmask.clone() {
        config.netmask(netmask);
    }
    if let Some(name) = CONFIG.tun_name.clone() {
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
    let mut read_buf = ReadBuf::new(&mut buf_memory);
    let mut recv_stream = Box::pin(ReceiverStream::new(tun_recv));

    poll_fn::<anyhow::Result<!>, _>(|cx| {
        if let Poll::Ready(_) = device.as_mut().poll_read(cx, &mut read_buf) {
            let data: Arc<[u8]> = Arc::from(read_buf.filled());
            debug!("Sending {} bytes from TUN", data.len());
            let _ = peers_send.send(PeersMessage::TunPacket(data));
            read_buf.clear();
            cx.waker().wake_by_ref();
        }
        if let Poll::Ready(Some(message)) = recv_stream.as_mut().poll_next(cx) {
            match message {
                TunMessage::Packet(data) => {
                    debug!("Sending {} bytes to TUN", data.len());
                    let _ = (&mut device as &mut Device).write_all(&data);
                }
            }
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    })
    .await?;
}
