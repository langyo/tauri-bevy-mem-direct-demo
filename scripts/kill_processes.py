"""Kill processes by name, cross-platform."""
import subprocess
import sys


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <name1> [name2] ...", file=sys.stderr)
        sys.exit(0)

    for name in sys.argv[1:]:
        try:
            if sys.platform == "win32":
                subprocess.call(
                    ["taskkill", "/F", "/IM", name],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
            else:
                subprocess.call(
                    ["pkill", "-f", name],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
        except FileNotFoundError:
            pass


if __name__ == "__main__":
    main()
