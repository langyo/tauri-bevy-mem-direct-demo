use std::rc::Rc;

use tairitsu_macros::rsx;
use tairitsu_vdom::VNode;

use crate::hooks::use_ws::CubeSensorData;

pub fn render_trend_chart(cube_id: String, data: CubeSensorData, on_close: Rc<dyn Fn()>) -> VNode {
    let label = if data.label.is_empty() {
        cube_id.replace("cube_", "节点")
    } else {
        data.label.clone()
    };

    let title = format!("{} — 温湿度趋势", label);
    let history = &data.history;

    let temp_display = if !history.is_empty() {
        let last = &history[history.len() - 1];
        format!("{:.1}°C", last.temperature)
    } else {
        "--".to_string()
    };

    let humi_display = if !history.is_empty() {
        let last = &history[history.len() - 1];
        format!("{:.1}%", last.humidity)
    } else {
        "--".to_string()
    };

    let history_len = history.len();

    let history_text = format!("已采集 {} 个数据点", history_len);
    let on_close_bg = on_close.clone();
    let on_close_btn = on_close.clone();

    rsx! {
        div {
            style: "position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.7);display:flex;align-items:center;justify-content:center;z-index:100;",
            onclick: move |e: tairitsu_vdom::events::MouseEvent| {
                if e.client_x == 0 && e.client_y == 0 {
                    return;
                }
                on_close_bg();
            },

            div {
                style: "width:600px;background:rgba(50,50,50,0.9);border-radius:12px;border:1px solid rgba(255,255,255,0.15);box-shadow:0 8px 32px rgba(0,0,0,0.5);overflow:hidden;",
                onclick: move |e: tairitsu_vdom::events::MouseEvent| {
                    e.stop_propagation();
                },

                div {
                    style: "display:flex;align-items:center;justify-content:space-between;padding:16px 20px;border-bottom:1px solid rgba(255,255,255,0.08);",

                    span {
                        style: "font-size:16px;font-weight:600;color:#ffffff;",
                        title
                    }

                    div {
                        style: "cursor:pointer;padding:4px 12px;border-radius:4px;background:rgba(255,107,107,0.15);color:#ff6b6b;font-size:13px;",
                        onclick: move |_: tairitsu_vdom::events::MouseEvent| {
                            on_close_btn();
                        },
                        "关闭"
                    }
                }

                div {
                    style: "padding:20px;",

                    div {
                        style: "display:flex;gap:24px;margin-bottom:16px;",

                        div {
                        style: "flex:1;padding:16px;border-radius:8px;background:rgba(80,80,80,0.4);text-align:center;",
                        div {
                            style: "font-size:12px;color:rgba(255,255,255,0.7);margin-bottom:8px;",
                            "当前温度"
                        }
                        div {
                            style: "font-size:28px;font-weight:bold;color:#ffffff;",
                                "{temp_display}"
                            }
                        }

                        div {
                        style: "flex:1;padding:16px;border-radius:8px;background:rgba(80,80,80,0.4);text-align:center;",
                        div {
                            style: "font-size:12px;color:rgba(255,255,255,0.7);margin-bottom:8px;",
                            "当前湿度"
                        }
                        div {
                            style: "font-size:28px;font-weight:bold;color:#ffffff;",
                                "{humi_display}"
                            }
                        }
                    }

                    div {
                        style: "font-size:13px;color:rgba(255,255,255,0.5);text-align:center;",
                        if history_len < 2 {
                            "数据采样中，等待更多数据点..."
                        } else {
                            "{history_text}"
                        }
                    }
                }

                div {
                    style: "display:flex;gap:24px;padding:0 20px 16px;",

                    div {
                        style: "display:flex;align-items:center;gap:6px;",
                        div {
                            style: "width:12px;height:3px;background:#ffffff;border-radius:2px;",
                        }
                        span {
                            style: "font-size:12px;color:rgba(255,255,255,0.7);",
                            "温度 (°C)"
                        }
                    }

                    div {
                        style: "display:flex;align-items:center;gap:6px;",
                        div {
                            style: "width:12px;height:3px;background:rgba(255,255,255,0.6);border-radius:2px;",
                        }
                        span {
                            style: "font-size:12px;color:rgba(255,255,255,0.7);",
                            "湿度 (%)"
                        }
                    }
                }
            }
        }
    }
}
