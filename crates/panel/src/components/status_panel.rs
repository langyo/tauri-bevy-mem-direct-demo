use tairitsu_macros::rsx;
use tairitsu_vdom::VNode;

use crate::hooks::use_ws::CubeSensorData;

pub fn render_status_panel(
    cube_id: String,
    data: CubeSensorData,
    selected: bool,
    on_select: impl Fn() + 'static,
) -> VNode {
    let label = if data.label.is_empty() {
        cube_id.replace("cube_", "节点")
    } else {
        data.label.clone()
    };

    let temp = data.temperature;
    let humidity = data.humidity;
    let ts = data.timestamp;

    let temp_color = if temp > 30.0 {
        "#ff9999"
    } else if temp > 26.0 {
        "#ffcc80"
    } else {
        "#ffffff"
    };

    let hum_color = if humidity > 70.0 {
        "#ff9999"
    } else if humidity > 60.0 {
        "#ffcc80"
    } else {
        "#ffffff"
    };

    let border_style = if selected {
        "border-color:rgba(255,255,255,0.6);box-shadow:0 0 12px rgba(255,255,255,0.2);"
    } else {
        "border-color:rgba(255,255,255,0.12);box-shadow:none;"
    };

    let ts_display = if ts > 0 {
        format_timestamp(ts)
    } else {
        "--".to_string()
    };

    let card_style = format!(
        "padding:12px;border-radius:8px;background:rgba(60,60,60,0.5);border:1px solid rgba(255,255,255,0.12);cursor:pointer;transition:all 0.2s;{}",
        border_style
    );
    let temp_style = format!("font-size:20px;font-weight:bold;color:{};", temp_color);
    let hum_style = format!("font-size:20px;font-weight:bold;color:{};", hum_color);
    let ts_text = format!("更新: {}", ts_display);

    rsx! {
        div {
            style: card_style,
            onclick: move |_: tairitsu_vdom::events::MouseEvent| {
                on_select();
            },

            div {
                style: "display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;",

                span {
                    style: "font-size:14px;font-weight:600;color:#ffffff;",
                    "{label}"
                }

                span {
                    style: "font-size:11px;color:rgba(255,255,255,0.3);",
                    "{cube_id}"
                }
            }

            div {
                style: "display:flex;gap:12px;",

                div {
                    style: "flex:1;padding:8px;border-radius:6px;background:rgba(80,80,80,0.4);",

                    div {
                        style: "font-size:11px;color:rgba(255,255,255,0.7);margin-bottom:4px;",
                        "温度"
                    }

                    div {
                        style: temp_style,
                        "{temp:.1}°C"
                    }
                }

                div {
                    style: "flex:1;padding:8px;border-radius:6px;background:rgba(80,80,80,0.4);",

                    div {
                        style: "font-size:11px;color:rgba(255,255,255,0.7);margin-bottom:4px;",
                        "湿度"
                    }

                    div {
                        style: hum_style,
                        "{humidity:.1}%"
                    }
                }
            }

            div {
                style: "margin-top:8px;font-size:11px;color:rgba(255,255,255,0.5);text-align:right;",
                "{ts_text}"
            }
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    if ts <= 0 {
        return "--".to_string();
    }
    let secs = ts / 1000;
    let mins = secs / 60;
    let hours = mins / 60;
    format!("{:02}:{:02}:{:02}", hours % 24, mins % 60, secs % 60)
}
