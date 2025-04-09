use std::{collections::HashMap, sync::Arc};

use anyhow::bail;
use iroh::{Endpoint, NodeAddr, NodeId};
use tokio::sync::mpsc;

use crate::{
    ALPN,
    peer::{Peer, PeerMessage, connect_peer},
    tun::TunMessage,
};

#[derive(Debug)]
pub enum PeersMessage {
    Connect(NodeAddr),
    AddPeer(NodeId, Peer),
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
    let mut peers = HashMap::<NodeId, Peer>::new();

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
            PeersMessage::AddPeer(public_key, peer) => {
                peers.insert(public_key, peer);
            }
            PeersMessage::TunPacket(data) => {
                for peer in peers.values() {
                    let _ = peer
                        .peer_send()
                        .send(PeerMessage::Packet(data.clone()))
                        .await;
                }
            }
            PeersMessage::PeerPacket(data) => {
                let _ = tun_send.send(TunMessage::Packet(data)).await;
            }
            PeersMessage::Disconnect(public_key) => {
                peers.remove(&public_key);
            }
        }
    }
}
