use std::time::{SystemTime, UNIX_EPOCH};

use tairitsu_hooks::use_effect;
use tairitsu_vdom::Platform;
use tairitsu_web::WitPlatform;

pub struct UseClockHandle;

pub fn use_clock() -> UseClockHandle {
    use_effect(|| {
        let platform = match WitPlatform::new() {
            Ok(p) => p,
            Err(_) => return,
        };
        let _ = platform.set_timeout(
            Box::new(move || {
                loop_tick(0);
            }),
            500,
        );
    });

    UseClockHandle
}

fn loop_tick(retries: u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = ((secs / 3600) % 24) as u32;
    let m = ((secs / 60) % 60) as u32;
    let s = (secs % 60) as u32;
    let time_str = format!("{:02}:{:02}:{:02}", h, m, s);

    let platform = match WitPlatform::new() {
        Ok(p) => p,
        Err(_) => return,
    };

    if let Some(el) = platform.get_element_by_id("clock-display") {
        platform.set_inner_html(&el, time_str);
        let _ = platform.set_timeout(Box::new(move || loop_tick(0)), 1000);
    } else if retries < 20 {
        let _ = platform.set_timeout(Box::new(move || loop_tick(retries + 1)), 500);
    }
}
