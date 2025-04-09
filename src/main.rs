#![feature(never_type)]

mod config;
mod peers;
mod tun;
use crate::tun::run_tun;

use std::convert::identity;

use anyhow::anyhow;
use peers::run_peers;
use tokio::{select, signal::ctrl_c, sync::mpsc, task::JoinSet};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let (tun_send, tun_recv) = mpsc::channel(32);
    let (peers_send, peers_recv) = mpsc::channel(32);

    tokio::spawn(run_tun(tun_recv, peers_send));
    tokio::spawn(run_peers(peers_recv, tun_send));

    Ok(())
}
