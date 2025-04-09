use std::time::Duration;

use iroh::NodeAddr;
use tokio::{sync::mpsc, time::sleep};

use crate::{config::CONFIG, peers::PeersMessage};

pub async fn run_controller(peers_send: mpsc::Sender<PeersMessage>) -> anyhow::Result<()> {
    if let Some(iroh_peer_ids) = &CONFIG.iroh_peer_ids {
        loop {
            for node_id in iroh_peer_ids {
                let _ = peers_send
                    .send(PeersMessage::Connect(NodeAddr::new(*node_id)))
                    .await;
            }
            sleep(Duration::from_secs(10)).await;
        }
    }
    Ok(())
}
