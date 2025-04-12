use std::time::Duration;

use iroh::NodeAddr;
use tokio::{sync::mpsc, time::sleep};

use crate::{config::get_config, peers::PeersMessage};

pub async fn run_controller(peers_send: mpsc::Sender<PeersMessage>) -> anyhow::Result<()> {
    loop {
        for peer in &get_config().iroh.peers {
            let _ = peers_send
                .send(PeersMessage::Connect(NodeAddr::new(peer.node_id)))
                .await;
        }
        sleep(Duration::from_secs(10)).await;
    }
}
