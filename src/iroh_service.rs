use std::{sync::Arc, time::Duration};

use anyhow::bail;
use injector::{Injectable, Injected};
use iroh::{
    Endpoint, NodeAddr, NodeId,
    endpoint::{Incoming, RecvStream, SendStream},
};
use tokio::sync::{broadcast::error::RecvError, mpsc};
use tracing::warn;

use crate::{mailbox::Mailbox, tun_service::TunService};

const ALPN: &[u8] = b"iroh-vpn";

#[derive(Debug, Clone)]
pub enum IrohServiceMessage {
    Connect(NodeAddr),
}

#[derive(Debug, Clone)]
pub enum PeerMessage {
    Packet(Arc<[u8]>),
    Ping(NodeId, mpsc::Sender<()>),
    Disconnect(NodeId),
}

#[derive(Injectable)]
pub struct IrohService {
    pub mailbox: Mailbox<IrohServiceMessage, 16>,
    pub packet_broadcast: Mailbox<PeerMessage, 256>,
    tun_service: Injected<TunService>,
}

impl IrohService {
    pub async fn run(self: Injected<Self>) -> anyhow::Result<()> {
        let endpoint = Endpoint::builder()
            .alpns(vec![ALPN.to_vec()])
            .discovery_n0()
            .bind()
            .await?;
        tokio::select!(
            result = self.clone().message_loop(endpoint.clone()) => result,
            result = self.incoming_loop(endpoint) => result,
        )?;
        Ok(())
    }

    async fn message_loop(self: Injected<Self>, endpoint: Endpoint) -> anyhow::Result<()> {
        let mut receiver = self.mailbox.broadcast.subscribe();
        loop {
            match receiver.recv().await {
                Ok(IrohServiceMessage::Connect(node_addr)) => {
                    tokio::spawn(with_timeout(
                        self.clone().connect_to(endpoint.clone(), node_addr),
                        Duration::from_secs(10),
                    ));
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
        Ok(())
    }

    async fn connect_to(
        self: Injected<Self>,
        endpoint: Endpoint,
        node_addr: NodeAddr,
    ) -> anyhow::Result<()> {
        let _ = self.packet_broadcast.broadcast.send(PeerMessage::Disconnect(node_addr.node_id));
        let connection = endpoint.clone().connect(node_addr.clone(), ALPN).await?;
        let (mut send, recv) = connection.open_bi().await?;
        send.write(b"hi").await?;
        self.run_peer(node_addr.node_id, send, recv).await?;
        Ok(())
    }

    async fn incoming_loop(self: Injected<Self>, endpoint: Endpoint) -> anyhow::Result<()> {
        while let Some(incoming) = endpoint.accept().await {
            tokio::spawn(with_timeout(
                self.clone().handle_incoming(incoming),
                Duration::from_secs(5),
            ));
        }
        Ok(())
    }

    async fn handle_incoming(self: Injected<Self>, incoming: Incoming) -> anyhow::Result<()> {
        let connection = incoming.accept()?.await?;
        let node_id = connection.remote_node_id()?;
        let _ = self.packet_broadcast.broadcast.send(PeerMessage::Disconnect(node_id));
        let (send, mut recv) = connection.accept_bi().await?;
        let mut buf = [0u8; 2];
        recv.read_exact(&mut buf).await?;
        if &buf != b"hi" {
            bail!("meanie :c");
        }
        self.run_peer(node_id, send, recv).await?;
        Ok(())
    }

    async fn run_peer(
        self: Injected<Self>,
        peer_id: NodeId,
        mut send_stream: SendStream,
        mut recv_stream: RecvStream,
    ) -> anyhow::Result<()> {
        let mut message_receiver = self.packet_broadcast.broadcast.subscribe();
        loop {
            tokio::select! {
                result = message_receiver.recv() => match result {
                    Ok(PeerMessage::Packet(packet)) => {
                        if let Err(err) = send_stream.write_all(&packet).await {
                            warn!("Error when talking to {}: {}", peer_id.fmt_short(), err);
                        }
                    }
                    Ok(PeerMessage::Ping(node_id, pong)) => if node_id == peer_id {let _ = pong.send(()).await;},
                    Ok(PeerMessage::Disconnect(node_id)) => if node_id == peer_id {break;},
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                },
                result = recv_stream.read_chunk(1518, true) => match result {
                    Ok(Some(chunk)) =>
                    if let Err(err) = self.tun_service.packet_stream.broadcast.send(Arc::from(&chunk.bytes[..])) {
                        warn!("Error when talking to {}: {}", peer_id.fmt_short(), err);
                    } ,
                    Ok(None) => break,
                    Err(_) => todo!(),
                },
            };
            match message_receiver.recv().await? {
                PeerMessage::Packet(packet) => {
                    if let Err(err) = send_stream.write_all(&packet).await {
                        warn!("Error when talking to {}: {}", peer_id.fmt_short(), err);
                    }
                }
                PeerMessage::Disconnect(node_id) if node_id == peer_id => {
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

async fn with_timeout<T>(future: impl Future<Output = T>, timeout: Duration) -> Option<T> {
    tokio::select! {
        output = future => Some(output),
        _ = tokio::time::sleep(timeout) => None,
    }
}
