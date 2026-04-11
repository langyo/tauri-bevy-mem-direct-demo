use std::rc::Rc;

use tairitsu_hooks::use_signal;
use tairitsu_macros::rsx;
use tairitsu_vdom::VNode;

use crate::components::keyboard_listener::render_keyboard_listener;
use crate::components::status_panel::render_status_panel;
use crate::components::trend_chart::render_trend_chart;
use crate::components::video_player::render_video_player;
use crate::hooks::use_clock::use_clock;
use crate::hooks::use_ws::use_ws;

#[expect(unused_braces)]
pub fn app() -> VNode {
    let ws = use_ws("ws://localhost:18742/ws");
    let _clock = use_clock();
    let selected_cube = use_signal(|| None::<String>);

    let sensor_data = ws.data.clone();
    let send_pick = ws.send_pick.clone();
    let send_move = ws.send_move.clone();
    let send_render_resolution = ws.send_render_resolution.clone();
    let ws_connected = ws.connected.clone();
    let active_resolution = use_signal(|| None::<(u32, u32)>);

    rsx! {
        div {
            style: "position:relative;width:100vw;height:100vh;margin:0;padding:0;overflow:hidden;font-family:sans-serif;color:#ffffff;background:transparent;",

            { render_video_player(send_pick.clone()) }

            div {
                style: "position:absolute;top:0;left:0;right:0;display:flex;align-items:center;justify-content:space-between;padding:8px 16px;background:rgba(40,40,40,0.6);backdrop-filter:blur(8px);z-index:10;",

                span {
                    style: "font-size:16px;font-weight:bold;color:#ffffff;",
                    "Demo"
                }

                div {
                    style: "display:flex;flex-direction:column;align-items:flex-end;gap:2px;",
                    span {
                        style: "font-size:12px;color:#ffffff;",
                        if ws_connected.get() {
                            "● 已连接"
                        } else {
                            "○ 未连接"
                        }
                    }
                    div {
                        style: "display:flex;gap:8px;font-size:11px;font-family:monospace;",
                        span {
                            id: "js-fps-display",
                            style: "color:#ffffff;",
                            "JS: --"
                        }
                        span {
                            id: "renderer-fps-display",
                            style: "color:#ffffff;",
                            "Bevy: --"
                        }
                        span {
                            id: "resolution-display",
                            style: "color:#ffffff;",
                            "--×--"
                        }
                        span {
                            id: "clock-display",
                            style: "color:#ffffff;",
                            "--:--:--"
                        }
                    }
                }
            }

            div {
                style: "position:absolute;top:44px;right:0;bottom:0;width:380px;display:flex;flex-direction:column;overflow-y:auto;padding:12px;gap:8px;background:rgba(40,40,40,0.5);backdrop-filter:blur(6px);z-index:10;",

                for cube_id in ["cube_0", "cube_1", "cube_2", "cube_3", "cube_4", "cube_5"] {
                    {
                        let key = cube_id.to_string();
                        let data_opt = sensor_data.get().get(&key).cloned();
                        match data_opt {
                            Some(data) => {
                                let is_selected = selected_cube.get() == Some(key.clone());
                                let sel = selected_cube.clone();
                                let kid = key.clone();
                                render_status_panel(key.clone(), data, is_selected, move || {
                                    let current = sel.get().clone();
                                    if current == Some(kid.clone()) {
                                        sel.set(None);
                                    } else {
                                        sel.set(Some(kid.clone()));
                                    }
                                })
                            }
                            None => rsx! {
                                div {
                                    key: "{key}-empty",
                                    style: "padding:16px;border-radius:8px;background:rgba(60,60,60,0.5);border:1px solid rgba(255,255,255,0.1);text-align:center;color:rgba(255,255,255,0.5);font-size:13px;",
                                    "{cube_id} 等待数据..."
                                }
                            },
                        }
                    }
                }
            }

            { render_keyboard_listener(send_move.clone()) }

            div {
                style: "position:fixed;bottom:12px;right:400px;display:flex;gap:4px;z-index:20;",
                for (res, label) in [(None::<(u32, u32)>, "Native"), (Some((1024u32, 768u32)), "1024×768"), (Some((1280u32, 720u32)), "1280×720"), (Some((1920u32, 1080u32)), "1920×1080"), (Some((2560u32, 1440u32)), "2560×1440"), (Some((3840u32, 2160u32)), "3840×2160")] {
                    {
                        let is_active = active_resolution.get() == res;
                        let bg = if is_active { "rgba(100,180,255,0.8)" } else { "rgba(60,60,60,0.6)" };
                        let ar = active_resolution.clone();
                        let srr = send_render_resolution.clone();
                        rsx! {
                            button {
                                key: "{label}",
                                style: {format!("padding:4px 10px;border:1px solid rgba(255,255,255,0.2);border-radius:4px;background:{};color:#fff;font-size:12px;cursor:pointer;font-family:monospace;", bg)},
                                onclick: move |_| {
                                    tairitsu_web::WitPlatform::console_log(&format!("onclick fired: {:?}", res));
                                    ar.set(res);
                                    match res {
                                        Some((w, h)) => srr(w, h),
                                        None => srr(0, 0),
                                    }
                                },
                                "{label}"
                            }
                        }
                    }
                }
            }

            {
                let selected = selected_cube.get().clone();
                match selected {
                    Some(cube_id) => {
                        let data_opt = sensor_data.get().get(&cube_id).cloned();
                        let sel = selected_cube.clone();
                        match data_opt {
                            Some(data) => render_trend_chart(cube_id.clone(), data, Rc::new(move || { sel.set(None); })),
                            None => VNode::empty(),
                        }
                    }
                    None => VNode::empty(),
                }
            }
        }
    }
}
