#![feature(arbitrary_self_types)]
#![feature(never_type)]

mod config_service;
mod iroh_service;
pub mod mailbox;
mod tun_service;
mod receiver_stream;

use std::convert::identity;

use anyhow::anyhow;
use injector::{Injectable, Injected, Injector};
use iroh_service::IrohService;
use tokio::{select, signal::ctrl_c, task::JoinSet};
use tun_service::TunService;

#[derive(Injectable)]
pub struct AppService {
    tun_service: Injected<TunService>,
    iroh_service: Injected<IrohService>,
}

impl AppService {
    async fn run(&self) -> anyhow::Result<()> {
        let mut join_set = JoinSet::new();
        join_set.spawn(self.tun_service.clone().run());
        join_set.spawn(self.iroh_service.clone().run());

        let result: anyhow::Result<()> = select! {
            Some(result) = join_set.join_next() => result.map_err(Into::into).and_then(identity),
            _ = ctrl_c() => Err(anyhow!("Interrupted...")),
        };

        join_set.abort_all();

        while join_set.join_next().await.is_some() {}

        result
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut injector = Injector::default();
    tracing_subscriber::fmt().init();
    injector.get::<AppService>().run().await?;
    Ok(())
}
