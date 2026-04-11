"""Build the panel WASM component using tairitsu packager."""
import re
import shutil
import subprocess
import sys
import os


def find_tairitsu_root():
    """Find the tairitsu monorepo root by walking up from the panel Cargo.toml."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(script_dir)
    panel_cargo = os.path.join(root_dir, "crates", "panel", "Cargo.toml")
    if not os.path.isfile(panel_cargo):
        return None

    with open(panel_cargo, "r", encoding="utf-8") as f:
        for line in f:
            if line.startswith("tairitsu-vdom"):
                start = line.find("path = \"")
                if start >= 0:
                    start += len("path = \"")
                    end = line.index("\"", start)
                    rel = line[start:end]
                    resolved = os.path.normpath(
                        os.path.join(root_dir, "crates", "panel", rel)
                    )
                    return os.path.dirname(os.path.dirname(resolved))
    return None


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(script_dir)
    panel_dir = os.path.join(root_dir, "crates", "panel")
    dist_dir = os.path.join(root_dir, "dist")

    ext = ".exe" if sys.platform == "win32" else ""
    tairitsu = os.path.join(
        panel_dir, "target", "tairitsu-tools", "bin", f"tairitsu{ext}"
    )

    if not os.path.isfile(tairitsu):
        print(f"Error: tairitsu packager not found at {tairitsu}", file=sys.stderr)
        sys.exit(1)

    env = os.environ.copy()
    env.pop("RUSTC_WRAPPER", None)

    result = subprocess.run(
        [tairitsu, "build"],
        cwd=panel_dir,
        env=env,
    )

    if result.returncode != 0:
        sys.exit(result.returncode)

    wrapper_js = os.path.join(dist_dir, "component-wrapper", "panel.js")
    if os.path.isfile(wrapper_js):
        with open(wrapper_js, "r", encoding="utf-8") as f:
            content = f.read()
        original = content
        content = re.sub(
            r"if\s*\(!\(ret\s+instanceof\s+OutputStream\)\)\s*\{",
            "if (ret == null || typeof ret.blockingWriteAndFlush !== 'function') {",
            content,
        )
        content = re.sub(
            r"if\s*\(!\(ret\s+instanceof\s+InputStream\)\)\s*\{",
            "if (ret == null || typeof ret.blockingRead !== 'function') {",
            content,
        )
        if content != original:
            with open(wrapper_js, "w", encoding="utf-8") as f:
                f.write(content)
            print(f"Patched {wrapper_js}: replaced instanceof with duck-typing for WASI resources")

    tairitsu_root = find_tairitsu_root()
    if tairitsu_root:
        runtime_src = os.path.join(
            tairitsu_root,
            "packages",
            "browser-glue",
            "dist",
            "runtime.js",
        )
        glue_dst = os.path.join(dist_dir, "browser-glue", "__tairitsu_glue__.js")
        if os.path.isfile(runtime_src) and os.path.isfile(glue_dst):
            shutil.copy2(runtime_src, glue_dst)
            print(f"Patched {glue_dst} with latest browser-glue runtime.js")


if __name__ == "__main__":
    main()
