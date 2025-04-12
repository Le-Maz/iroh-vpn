use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{anyhow, bail};
use iroh::{Endpoint, NodeAddr, NodeId, endpoint::Incoming};
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    ALPN,
    config::{WHITELIST, get_config},
    peer::{Peer, PeerMessage, run_peer},
    tun::TunMessage,
};

#[derive(Debug)]
pub enum PeersMessage {
    Connect(NodeAddr),
    AddPeer(NodeId, Peer),
    TunPacket(Arc<[u8]>),
    PeerPacket(Vec<u8>),
    Disconnect(NodeId),
}

pub async fn run_peers(
    peers_send: mpsc::Sender<PeersMessage>,
    peers_recv: mpsc::Receiver<PeersMessage>,
    tun_send: mpsc::Sender<TunMessage>,
) -> anyhow::Result<!> {
    let endpoint = Endpoint::builder()
        .alpns(vec![ALPN.to_vec()])
        .discovery_n0()
        .secret_key(get_config().iroh.sk.clone())
        .bind()
        .await?;

    info!("Node ID: {}", endpoint.node_id());

    tokio::select! {
        result = run_incoming_loop(peers_send.clone(), endpoint.clone()) => result,
        result = run_message_loop(peers_send, peers_recv, tun_send, endpoint) => result,
    }?;
}

async fn run_message_loop(
    peers_send: mpsc::Sender<PeersMessage>,
    mut peers_recv: mpsc::Receiver<PeersMessage>,
    tun_send: mpsc::Sender<TunMessage>,
    endpoint: Endpoint,
) -> anyhow::Result<!> {
    let mut peers = HashMap::<NodeId, Peer>::new();
    loop {
        let message = peers_recv
            .recv()
            .await
            .ok_or(anyhow!("PeersMessage channel broken"))?;
        match message {
            PeersMessage::Connect(node_addr) if !peers.contains_key(&node_addr.node_id) => {
                tokio::spawn(connect_peer(
                    peers_send.clone(),
                    endpoint.clone(),
                    node_addr,
                ));
            }
            PeersMessage::AddPeer(node_id, peer) => {
                if peers.insert(node_id, peer).is_some() {
                    info!("Disconnected from {}", node_id.fmt_short());
                }
                info!("Connected to {}", node_id.fmt_short());
            }
            PeersMessage::TunPacket(data) => {
                let mut to_remove = Vec::with_capacity(2);
                for (node_id, peer) in peers.iter() {
                    if peer
                        .peer_send()
                        .send(PeerMessage::TunPacket(data.clone()))
                        .await
                        .is_err()
                    {
                        to_remove.push(*node_id);
                    }
                }
                for node_id in to_remove {
                    info!("Disconnected from {}", node_id.fmt_short());
                    peers.remove(&node_id);
                }
            }
            PeersMessage::PeerPacket(data) => {
                let _ = tun_send.send(TunMessage::PeerPacket(data)).await;
            }
            PeersMessage::Disconnect(node_id) => {
                info!("Disconnected from {}", node_id.fmt_short());
                peers.remove(&node_id);
            }
            _ => {}
        }
    }
}

async fn run_incoming_loop(
    peers_send: mpsc::Sender<PeersMessage>,
    endpoint: Endpoint,
) -> anyhow::Result<!> {
    loop {
        let incoming = endpoint.accept().await.ok_or(anyhow!("Endpoint closed"))?;
        tokio::spawn(with_timeout(
            handle_incoming(peers_send.clone(), incoming),
            Duration::from_secs(5),
        ));
    }
}

async fn handle_incoming(
    peers_send: mpsc::Sender<PeersMessage>,
    incoming: Incoming,
) -> anyhow::Result<()> {
    let connection = incoming.accept()?.await?;
    let node_id = connection.remote_node_id()?;
    if !WHITELIST.contains(&node_id) {
        bail!("Peer outside of whitelist tried to connect");
    }
    let (peer_send, peer_recv) = mpsc::channel(16);
    let abort_handle =
        tokio::spawn(run_peer(peer_recv, peers_send.clone(), connection)).abort_handle();

    let peer = Peer::new(peer_send, abort_handle);

    peers_send
        .send(PeersMessage::AddPeer(node_id, peer))
        .await?;
    Ok(())
}

async fn connect_peer(
    peers_send: mpsc::Sender<PeersMessage>,
    endpoint: Endpoint,
    node_addr: NodeAddr,
) -> anyhow::Result<()> {
    let connection = endpoint.connect(node_addr.clone(), ALPN).await?;
    let (peer_send, peer_recv) = mpsc::channel(16);
    let abort_handle =
        tokio::spawn(run_peer(peer_recv, peers_send.clone(), connection)).abort_handle();

    let peer = Peer::new(peer_send, abort_handle);

    peers_send
        .send(PeersMessage::AddPeer(node_addr.node_id, peer))
        .await?;
    Ok(())
}

async fn with_timeout<T>(future: impl Future<Output = T>, timeout: Duration) -> Option<T> {
    tokio::select! {
        output = future => Some(output),
        _ = tokio::time::sleep(timeout) => None,
    }
}
