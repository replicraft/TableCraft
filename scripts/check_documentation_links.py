"""Check local Markdown targets without inspecting ignored private documents."""
import argparse
from bisect import bisect_right
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
INLINE_TARGET = re.compile(r"\]\(\s*(<[^>\n]+>|[^)\s]+)")
REFERENCE_TARGET = re.compile(r"^[ \t]*\[(?!\^)[^\]\n]+\]:[ \t]*(<[^>\n]+>|\S+)", re.MULTILINE)
HTML_TARGET = re.compile(r"\b(?:href|src)=[\"']([^\"']+)[\"']")


def check_documents(root, sources):
    checked = 0
    missing = []
    for source in sorted(sources):
        path = root / source
        if not path.is_file():
            continue
        content = path.read_text(encoding="utf-8")
        line_starts = [0, *[match.end() for match in re.finditer("\n", content)]]
        for pattern in (INLINE_TARGET, REFERENCE_TARGET, HTML_TARGET):
            for match in pattern.finditer(content):
                target = match[1].strip("<>")
                parsed = urlsplit(target)
                if parsed.scheme or parsed.netloc or not parsed.path:
                    continue
                relative = unquote(parsed.path)
                if relative.startswith("/"):
                    destination = root / relative.lstrip("/")
                else:
                    destination = path.parent / relative
                checked += 1
                if not destination.exists():
                    missing.append((source, bisect_right(line_starts, match.start()), target))
    return checked, sorted(missing)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    names = subprocess.check_output(
        [
            "git", "-C", str(args.root), "ls-files", "-z", "--cached",
            "--others", "--exclude-standard", "--", "*.md",
        ],
        text=True,
    ).split("\0")
    checked, missing = check_documents(args.root, {name for name in names if name})
    for source, line, target in missing:
        print(f"{source}:{line}: missing local target {target}")
    print(f"Checked {checked} local documentation targets; {len(missing)} missing.")
    raise SystemExit(bool(missing))


if __name__ == "__main__":
    main()
