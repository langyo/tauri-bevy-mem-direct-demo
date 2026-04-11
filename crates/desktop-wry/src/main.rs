#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("demo-panel-wry currently supports Windows WebView2 zero-copy path only");
}

#[cfg(target_os = "windows")]
mod sidecar;

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "windows")]
fn main() {
    win::main();
}
