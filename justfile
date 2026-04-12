set shell := ["powershell", "-NoProfile", "-Command"]

_python := if os() == "windows" { "python" } else { "python3" }

default: dev

_renderer_src_debug := if os() == "windows" { "target/debug/renderer.exe" } else { "target/debug/renderer" }
_renderer_dst_debug := if os() == "windows" { "target/debug/binaries/renderer.exe" } else { "target/debug/binaries/renderer" }
_renderer_src_release := if os() == "windows" { "target/release/renderer.exe" } else { "target/release/renderer" }
_renderer_dst_release := if os() == "windows" { "target/release/binaries/renderer.exe" } else { "target/release/binaries/renderer" }

_copy-renderer-debug:
    {{ _python }} scripts/copy_file.py {{ _renderer_src_debug }} {{ _renderer_dst_debug }}

_copy-renderer-release:
    {{ _python }} scripts/copy_file.py {{ _renderer_src_release }} {{ _renderer_dst_release }}

build-panel:
    {{ _python }} scripts/build_panel.py

_kill-old:
    {{ _python }} scripts/kill_processes.py renderer.exe renderer demo-panel.exe demo-panel demo-panel-wry.exe demo-panel-wry demo-panel-cef.exe demo-panel-cef

dev mode="native": _kill-old build-renderer-debug _copy-renderer-debug build-panel
    if ("{{mode}}" -eq "wsl") { $p = $PWD.Path -replace '\\','/'; $linux_path = '/mnt/' + $p.Substring(0,1).ToLower() + $p.Substring(2); wsl.exe bash -lc "cd '$linux_path' && cargo run --package demo-panel-wry" } elseif ("{{mode}}" -eq "cef") { cargo run --package demo-panel-cef } else { cargo run --package demo-panel }

dev-wry:
    just dev wsl

build mode="native": build-renderer-release _copy-renderer-release build-panel
    if ("{{mode}}" -eq "wsl") { $p = $PWD.Path -replace '\\','/'; $linux_path = '/mnt/' + $p.Substring(0,1).ToLower() + $p.Substring(2); wsl.exe bash -lc "cd '$linux_path' && cargo build --package demo-panel-wry --release" } elseif ("{{mode}}" -eq "cef") { cargo build --package demo-panel-cef --release } else { cargo build --package demo-panel --release }

dev-cef:
    just dev cef

build-cef:
    just build cef

build-wry:
    just build wsl

build-renderer-debug:
    cargo build --package renderer

build-renderer-release:
    cargo build --package renderer --release

check:
    cargo check --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace -- -D warnings

clean:
    cargo clean
    {{ _python }} -c "import shutil, os; p='dist'; shutil.rmtree(p) if os.path.isdir(p) else None"
