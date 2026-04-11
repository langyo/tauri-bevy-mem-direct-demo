use std::rc::Rc;

use tairitsu_macros::rsx;
use tairitsu_vdom::VNode;

pub fn render_keyboard_listener(on_move: Rc<dyn Fn(String)>) -> VNode {
    rsx! {
        div {
            style: "position:fixed;top:0;left:0;right:0;bottom:0;z-index:9999;outline:none;pointer-events:none;",
            tabindex: "0",
            onkeydown: move |e: tairitsu_vdom::events::KeyboardEvent| {
                let direction = match e.key.as_str() {
                    "w" | "W" | "ArrowUp" => Some("forward".to_string()),
                    "s" | "S" | "ArrowDown" => Some("backward".to_string()),
                    "a" | "A" | "ArrowLeft" => Some("left".to_string()),
                    "d" | "D" | "ArrowRight" => Some("right".to_string()),
                    _ => None,
                };

                if let Some(dir) = direction {
                    e.prevent_default();
                    on_move(dir);
                }
            },
        }
    }
}
