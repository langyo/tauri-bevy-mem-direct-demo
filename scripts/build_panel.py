"""Build panel frontend by copying pure JS static assets to dist/."""
import os
import shutil
import sys


def copy_tree(src_dir: str, dst_dir: str) -> None:
    for root, dirs, files in os.walk(src_dir):
        rel = os.path.relpath(root, src_dir)
        out_root = dst_dir if rel == "." else os.path.join(dst_dir, rel)
        os.makedirs(out_root, exist_ok=True)
        for d in dirs:
            os.makedirs(os.path.join(out_root, d), exist_ok=True)
        for f in files:
            shutil.copy2(os.path.join(root, f), os.path.join(out_root, f))


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(script_dir)
    src_dir = os.path.join(root_dir, "crates", "panel", "web")
    dist_dir = os.path.join(root_dir, "dist")

    if not os.path.isdir(src_dir):
        print(f"Error: panel web assets not found at {src_dir}", file=sys.stderr)
        sys.exit(1)

    if os.path.isdir(dist_dir):
        shutil.rmtree(dist_dir)
    os.makedirs(dist_dir, exist_ok=True)
    copy_tree(src_dir, dist_dir)
    print(f"Panel static assets copied: {src_dir} -> {dist_dir}")


if __name__ == "__main__":
    main()
