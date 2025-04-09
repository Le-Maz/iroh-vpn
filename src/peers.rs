use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
    time::Duration,
};

use anyhow::anyhow;
use iroh::{Endpoint, NodeAddr, NodeId, PublicKey, SecretKey, endpoint::Incoming};
use rand::rngs::OsRng;
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    ALPN,
    config::CONFIG,
    peer::{Peer, PeerMessage, run_peer},
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

fn generate_sk() -> SecretKey {
    let mut rng = OsRng::default();
    SecretKey::generate(&mut rng)
}

static SECRET_KEY: LazyLock<SecretKey> = LazyLock::new(|| {
    CONFIG
        .iroh_sk_path
        .clone()
        .map(|sk_path| match std::fs::exists(&sk_path).unwrap() {
            true => {
                let sk_string = std::fs::read_to_string(sk_path).unwrap();
                sk_string.parse().unwrap()
            }
            false => {
                let sk = generate_sk();
                std::fs::write(sk_path, sk.to_string()).unwrap();
                sk
            }
        })
        .unwrap_or_else(generate_sk)
});

pub async fn run_peers(
    peers_send: mpsc::Sender<PeersMessage>,
    peers_recv: mpsc::Receiver<PeersMessage>,
    tun_send: mpsc::Sender<TunMessage>,
) -> anyhow::Result<!> {
    let endpoint = Endpoint::builder()
        .alpns(vec![ALPN.to_vec()])
        .discovery_n0()
        .secret_key(SECRET_KEY.clone())
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

static WHITELIST: LazyLock<Option<HashSet<PublicKey>>> = LazyLock::new(|| {
    CONFIG
        .iroh_peer_ids
        .as_ref()
        .map(|peer_ids| peer_ids.into_iter().cloned().collect())
});

async fn handle_incoming(
    peers_send: mpsc::Sender<PeersMessage>,
    incoming: Incoming,
) -> anyhow::Result<()> {
    let connection = incoming.accept()?.await?;
    let node_id = connection.remote_node_id()?;
    if WHITELIST
        .as_ref()
        .map(|peer_ids| peer_ids.contains(&node_id))
        .unwrap_or(true)
    {
        info!("Connected to {}", connection.remote_node_id()?.fmt_short());
        let (peer_send, peer_recv) = mpsc::channel(16);
        let abort_handle =
            tokio::spawn(run_peer(connection, peer_recv, peers_send.clone())).abort_handle();

        let peer = Peer::new(peer_send, abort_handle);

        peers_send
            .send(PeersMessage::AddPeer(node_id, peer))
            .await?;
    }
    Ok(())
}

async fn connect_peer(
    peers_send: mpsc::Sender<PeersMessage>,
    endpoint: Endpoint,
    node_addr: NodeAddr,
) -> anyhow::Result<()> {
    let connection = endpoint.connect(node_addr.clone(), ALPN).await?;
    info!("Connected to {}", connection.remote_node_id()?.fmt_short());
    let (peer_send, peer_recv) = mpsc::channel(16);
    let abort_handle =
        tokio::spawn(run_peer(connection, peer_recv, peers_send.clone())).abort_handle();

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
