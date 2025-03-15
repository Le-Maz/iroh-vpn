use injector::{Injected, Injector};
use injector_macro::Injectable;

#[derive(Injectable)]
pub struct ConfigService {}

#[derive(Injectable)]
pub struct TuiService {}

#[derive(Injectable)]
pub struct TunService {}

#[derive(Injectable)]
pub struct IrohService {}

#[derive(Injectable)]
pub struct AppService {
    config_service: Injected<ConfigService>,
    tui_service: Injected<TuiService>,
    tun_service: Injected<TunService>,
    iroh_service: Injected<IrohService>,
}

impl AppService {
    pub fn run(&self) {}
}

#[tokio::main]
async fn main() {
    let mut injector = Injector::default();
    injector.get::<AppService>().run();
}
