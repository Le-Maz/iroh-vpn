use std::sync::Arc;

use anyhow::bail;
use iroh::{
    NodeId,
    endpoint::{Connection, RecvStream, SendStream},
};
use tokio::{sync::mpsc, task::AbortHandle};
use tracing::debug;

use crate::peers::PeersMessage;

#[derive(Debug)]
pub struct Peer {
    peer_send: mpsc::Sender<PeerMessage>,
    abort_handle: AbortHandle,
}

impl Peer {
    pub fn new(peer_send: mpsc::Sender<PeerMessage>, abort_handle: AbortHandle) -> Self {
        Self {
            peer_send,
            abort_handle,
        }
    }

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

pub async fn run_peer(
    peer_recv: mpsc::Receiver<PeerMessage>,
    peers_send: mpsc::Sender<PeersMessage>,
    node_id: NodeId,
    connection: Connection,
) -> anyhow::Result<!> {
    tokio::select! {
        result = async {
            let send_stream = connection.open_uni().await?;
            receive_messages(peer_recv, send_stream, node_id.clone()).await?;
        } => result,
        result = async {
            let recv_stream = connection.accept_uni().await?;
            send_messages(peers_send, recv_stream, node_id).await?;
        } => result,
    }
}

async fn receive_messages(
    mut peer_recv: mpsc::Receiver<PeerMessage>,
    mut send_stream: SendStream,
    node_id: NodeId,
) -> anyhow::Result<!> {
    loop {
        let Some(message) = peer_recv.recv().await else {
            bail!("PeerMessage channel broken");
        };
        match message {
            PeerMessage::Packet(data) => {
                debug!("Sending {} bytes to {}", data.len(), node_id.fmt_short());
                send_stream.write_all(&data).await?;
            }
        }
    }
}

async fn send_messages(
    peers_send: mpsc::Sender<PeersMessage>,
    mut recv_stream: RecvStream,
    node_id: NodeId,
) -> anyhow::Result<!> {
    loop {
        let mut buf = [0u8; 1518];
        let Some(size) = recv_stream.read(&mut buf).await? else {
            bail!("Connection broken");
        };
        debug!("Received {} bytes from {}", size, node_id.fmt_short());
        peers_send
            .send(PeersMessage::PeerPacket(Arc::from(&buf[..size])))
            .await?;
    }
}
