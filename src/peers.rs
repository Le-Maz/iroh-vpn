use std::{sync::Arc, time::Duration};

use iroh::{NodeAddr, NodeId};
use tokio::sync::mpsc;

use crate::tun::TunMessage;

const ALPN: &[u8] = b"iroh-vpn";

#[derive(Debug)]
pub enum PeersMessage {
    Connect(NodeAddr),
    Packet(Arc<[u8]>),
    Disconnect(NodeId),
}

pub async fn run_peers(
    peers_recv: mpsc::Receiver<PeersMessage>,
    tun_send: mpsc::Sender<TunMessage>,
) -> anyhow::Result<()> {
    tun_send
        .send(TunMessage::Packet(Arc::from([0; 1024])))
        .await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok(())
}
