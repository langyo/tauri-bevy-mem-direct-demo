mod app;
mod components;
mod hooks;

use anyhow::Result;
use tairitsu_vdom::Platform;
use tairitsu_web::{init_logger, log::LevelFilter, WitPlatform};

pub fn run_app() -> Result<()> {
    let _ = init_logger(LevelFilter::Info);
    let platform = WitPlatform::new()?;
    let vnode = app::app();
    platform.mount_vnode_to_app(&vnode)?;

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn tairitsu_component_bootstrap() {
    let _ = run_app();
}
