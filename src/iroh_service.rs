use std::sync::Arc;

use injector::{Injected, Injectable};

use crate::mailbox::Mailbox;

#[derive(Debug)]
pub enum IrohServiceMessage {
    Packet(Arc<[u8]>),
}

#[derive(Injectable)]
pub struct IrohService {
    pub mailbox: Mailbox<IrohServiceMessage>,
}

impl IrohService {
    pub async fn run(self: Injected<Self>) -> anyhow::Result<()> {
        loop {
            tokio::task::yield_now().await;
        }
    }
}
