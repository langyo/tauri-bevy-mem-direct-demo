use tairitsu_macros::rsx;
use tairitsu_vdom::VNode;

pub fn render_video_player(_send_pick: std::rc::Rc<dyn Fn(f32, f32)>) -> VNode {
    rsx! {
        canvas {
            id: "bevy-canvas",
            style: "position:absolute;top:0;left:0;width:100%;height:100%;display:block;z-index:0;",
        }
    }
}
