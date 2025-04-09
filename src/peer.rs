use std::sync::Arc;

use anyhow::bail;
use iroh::{
    Endpoint, NodeAddr,
    endpoint::{Connection, RecvStream, SendStream},
};
use tokio::{sync::mpsc, task::AbortHandle};
use tracing::info;

use crate::{ALPN, peers::PeersMessage};

pub async fn connect_peer(
    peers_send: mpsc::Sender<PeersMessage>,
    endpoint: Endpoint,
    node_addr: NodeAddr,
) -> anyhow::Result<()> {
    let connection = endpoint.connect(node_addr.clone(), ALPN).await?;
    info!("Connected to {}", connection.remote_node_id()?.fmt_short());
    let (peer_send, peer_recv) = mpsc::channel(16);
    let abort_handle =
        tokio::spawn(run_peer(connection, peer_recv, peers_send.clone())).abort_handle();

    let peer_meta = Peer {
        peer_send,
        abort_handle,
    };
    peers_send
        .send(PeersMessage::AddPeer(node_addr.node_id, peer_meta))
        .await?;
    Ok(())
}

#[derive(Debug)]
pub struct Peer {
    peer_send: mpsc::Sender<PeerMessage>,
    abort_handle: AbortHandle,
}

impl Peer {
    pub fn peer_send(&self) -> &mpsc::Sender<PeerMessage> {
        &self.peer_send
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

pub enum PeerMessage {
    Packet(Arc<[u8]>),
}

async fn run_peer(
    connection: Connection,
    peer_recv: mpsc::Receiver<PeerMessage>,
    peers_send: mpsc::Sender<PeersMessage>,
) -> anyhow::Result<!> {
    tokio::select! {
        result = async {
            let send_stream = connection.open_uni().await?;
            receive_messages(peer_recv, send_stream).await?;
        } => result,
        result = async {
            let recv_stream = connection.accept_uni().await?;
            send_messages(peers_send, recv_stream).await?;
        } => result,
    }
}

async fn receive_messages(
    mut peer_recv: mpsc::Receiver<PeerMessage>,
    mut send_stream: SendStream,
) -> anyhow::Result<!> {
    loop {
        let Some(message) = peer_recv.recv().await else {
            bail!("PeerMessage channel broken");
        };
        match message {
            PeerMessage::Packet(data) => send_stream.write_all(&data).await?,
        }
    }
}

async fn send_messages(
    peers_send: mpsc::Sender<PeersMessage>,
    mut recv_stream: RecvStream,
) -> anyhow::Result<!> {
    loop {
        let mut buf = [0u8; 1518];
        let Some(size) = recv_stream.read(&mut buf).await? else {
            bail!("Connection broken");
        };
        peers_send
            .send(PeersMessage::PeerPacket(Arc::from(&buf[..size])))
            .await?;
    }
}
