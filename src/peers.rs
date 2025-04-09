use std::{collections::HashMap, sync::Arc};

use anyhow::bail;
use iroh::{Endpoint, NodeAddr, NodeId};
use tokio::sync::mpsc;

use crate::{
    peer::{PeerMessage, run_peer},
    tun::TunMessage,
};

const ALPN: &[u8] = b"iroh-vpn";

#[derive(Debug)]
pub enum PeersMessage {
    Connect(NodeAddr),
    AddPeer(NodeId, mpsc::Sender<PeerMessage>),
    TunPacket(Arc<[u8]>),
    PeerPacket(Arc<[u8]>),
    Disconnect(NodeId),
}

pub async fn run_peers(
    peers_send: mpsc::Sender<PeersMessage>,
    mut peers_recv: mpsc::Receiver<PeersMessage>,
    tun_send: mpsc::Sender<TunMessage>,
) -> anyhow::Result<!> {
    let endpoint = Endpoint::builder()
        .alpns(vec![ALPN.to_vec()])
        .discovery_n0()
        .bind()
        .await?;
    let mut peers = HashMap::<NodeId, mpsc::Sender<PeerMessage>>::new();

    loop {
        let Some(message) = peers_recv.recv().await else {
            bail!("PeersMessage channel broken")
        };
        match message {
            PeersMessage::Connect(node_addr) => {
                tokio::spawn(connect_peer(
                    peers_send.clone(),
                    endpoint.clone(),
                    node_addr,
                ));
            }
            PeersMessage::AddPeer(public_key, peer_send) => {
                if let Some(previous) = peers.insert(public_key, peer_send) {
                    let _ = previous.send(PeerMessage::Disconnect).await;
                }
            }
            PeersMessage::TunPacket(data) => {
                for peer in peers.values() {
                    let _ = peer.send(PeerMessage::Packet(data.clone())).await;
                }
            }
            PeersMessage::PeerPacket(data) => {
                let _ = tun_send.send(TunMessage::Packet(data)).await;
            }
            PeersMessage::Disconnect(public_key) => {
                peers.get(&public_key);
            }
        }
    }
}

async fn connect_peer(
    peers_send: mpsc::Sender<PeersMessage>,
    endpoint: Endpoint,
    node_addr: NodeAddr,
) -> anyhow::Result<()> {
    let connection = endpoint.connect(node_addr.clone(), ALPN).await?;
    let (peer_send, peer_recv) = mpsc::channel(16);
    peers_send
        .send(PeersMessage::AddPeer(node_addr.node_id, peer_send))
        .await?;
    tokio::spawn(run_peer(connection, peer_recv, peers_send));
    Ok(())
}
