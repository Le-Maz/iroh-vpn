#![feature(arbitrary_self_types)]

mod config_service;
pub mod mailbox;
mod tun_service;

use injector::{Injected, Injector};
use injector_macro::Injectable;
use tokio::task::JoinSet;
use tun_service::TunService;

#[derive(Injectable)]
pub struct TuiService {}

#[derive(Injectable)]
pub struct IrohService {}

#[derive(Injectable)]
pub struct AppService {
    tui_service: Injected<TuiService>,
    tun_service: Injected<TunService>,
    iroh_service: Injected<IrohService>,
}

impl AppService {
    async fn run(&self) -> anyhow::Result<()> {
        let mut join_set = JoinSet::new();
        join_set.spawn(self.tun_service.clone().run());
        while let Some(result) = join_set.join_next().await {
            result??;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut injector = Injector::default();
    injector.get::<AppService>().run().await?;
    Ok(())
}
