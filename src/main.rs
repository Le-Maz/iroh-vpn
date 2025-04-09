#![feature(never_type)]

mod config;
mod peer;
mod peers;
mod tun;
use crate::tun::run_tun;

use peers::run_peers;
use tokio::{signal::ctrl_c, sync::mpsc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let (tun_send, tun_recv) = mpsc::channel(32);
    let (peers_send, peers_recv) = mpsc::channel(32);

    tokio::spawn(run_tun(tun_recv, peers_send.clone()));
    tokio::spawn(run_peers(peers_send, peers_recv, tun_send));

    ctrl_c().await?;

    Ok(())
}
