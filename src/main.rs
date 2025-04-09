#![feature(never_type)]

mod config;
mod controller;
mod peer;
mod peers;
mod tun;

use tokio::{signal::ctrl_c, sync::mpsc};

use crate::{controller::run_controller, peers::run_peers, tun::run_tun};

const ALPN: &[u8] = b"iroh-vpn";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let (tun_send, tun_recv) = mpsc::channel(32);
    let (peers_send, peers_recv) = mpsc::channel(32);

    let join_tun = tokio::spawn(run_tun(tun_recv, peers_send.clone()));
    let join_peers = tokio::spawn(run_peers(peers_send.clone(), peers_recv, tun_send));
    let join_controller = tokio::spawn(run_controller(peers_send));

    ctrl_c().await?;

    join_tun.abort();
    join_peers.abort();
    join_controller.abort();

    let _ = join_tun.await;
    let _ = join_peers.await;
    let _ = join_controller.await;

    Ok(())
}
