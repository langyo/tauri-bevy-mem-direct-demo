#[cfg(target_os = "windows")]
mod sidecar;

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn main() {
    eprintln!("demo-panel-wry supports windows/linux only");
}

#[cfg(target_os = "windows")]
fn main() {
    win::main();
}

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}
