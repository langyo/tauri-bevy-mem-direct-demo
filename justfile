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
    {{ _python }} scripts/kill_processes.py renderer.exe renderer demo-panel.exe demo-panel

dev: _kill-old build-renderer-debug _copy-renderer-debug build-panel
    cargo run --package demo-panel

build: build-renderer-release _copy-renderer-release build-panel
    cargo build --package demo-panel --release

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
