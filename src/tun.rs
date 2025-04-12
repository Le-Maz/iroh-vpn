use std::future::poll_fn;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::{Stream, pin_mut};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info};
use tun::{AsyncDevice, create_as_async};

use crate::config::TUN_CONFIG;
use crate::peers::PeersMessage;

pub enum TunMessage {
    PeerPacket(Vec<u8>),
}

pub async fn run_tun(
    tun_recv: mpsc::Receiver<TunMessage>,
    peers_send: mpsc::Sender<PeersMessage>,
) -> anyhow::Result<!> {
    let device = create_as_async(&TUN_CONFIG)?;
    let recv_stream = ReceiverStream::new(tun_recv);
    pin_mut!(device);
    pin_mut!(recv_stream);

    info!("Created TUN device");

    let mut buf_memory = [0u8; <u16>::MAX as usize];
    let mut read_buf = ReadBuf::new(&mut buf_memory);

    poll_fn::<anyhow::Result<!>, _>(|cx| {
        poll_device_read(&peers_send, &mut device, &mut read_buf, cx);
        if let Some(packet) = poll_actor_message(&mut recv_stream, cx) {
            let _ = device.as_mut().write_all(&packet);
        }
        Poll::Pending
    })
    .await?;
}

fn poll_device_read(
    peers_send: &mpsc::Sender<PeersMessage>,
    device: &mut Pin<&mut AsyncDevice>,
    read_buf: &mut ReadBuf<'_>,
    cx: &mut Context<'_>,
) {
    if device.as_mut().poll_read(cx, read_buf).is_ready() {
        let data: Arc<[u8]> = Arc::from(read_buf.filled());
        debug!("Sending {} bytes", data.len());
        read_buf.clear();
        let _ = peers_send.try_send(PeersMessage::TunPacket(data));
        cx.waker().wake_by_ref();
    }
}

fn poll_actor_message(
    recv_stream: &mut Pin<&mut ReceiverStream<TunMessage>>,
    cx: &mut Context<'_>,
) -> Option<Vec<u8>> {
    let Poll::Ready(Some(message)) = recv_stream.as_mut().poll_next(cx) else {
        return None;
    };
    cx.waker().wake_by_ref();
    match message {
        TunMessage::PeerPacket(data) => {
            debug!("Received {} bytes", data.len());
            Some(data)
        }
    }
}
