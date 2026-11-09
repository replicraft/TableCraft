"""Keep Rust module names consistent and crate roots limited to composition."""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]


def tokens(source):
    """Skip Rust comments and keep literals opaque, including nested comments."""
    position = 0
    while position < len(source):
        if source[position].isspace():
            position += 1
            continue
        if source.startswith("//", position):
            end = source.find("\n", position)
            position = len(source) if end < 0 else end + 1
            continue
        if source.startswith("/*", position):
            depth = 1
            position += 2
            while position < len(source) and depth:
                if source.startswith("/*", position):
                    depth += 1
                    position += 2
                elif source.startswith("*/", position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
            if depth:
                raise ValueError("unterminated block comment")
            continue
        raw = re.match(r'(?:br|cr|r)(#*)"', source[position:])
        if raw:
            terminator = '"' + raw.group(1)
            end = source.find(terminator, position + raw.end())
            if end < 0:
                raise ValueError("unterminated raw string")
            position = end + len(terminator)
            yield "literal"
            continue
        if source[position] == '"':
            position += 1
            while position < len(source):
                if source[position] == "\\":
                    position += 2
                elif source[position] == '"':
                    position += 1
                    break
                else:
                    position += 1
            else:
                raise ValueError("unterminated string")
            yield "literal"
            continue
        word = re.match(r"[A-Za-z_][A-Za-z_0-9]*", source[position:])
        if word:
            yield word.group()
            position += word.end()
        else:
            yield source[position]
            position += 1


def composition_error(source):
    """Accept attributes, module declarations, imports and explicit exports only.

    This deliberately small source guard is not a Rust parser or type checker.
    The compiler remains responsible for syntax, visibility and module resolution.
    """
    words = list(tokens(source))
    index = 0
    while index < len(words):
        if words[index] == "#":
            index += 1
            if index < len(words) and words[index] == "!":
                index += 1
            if index >= len(words) or words[index] != "[":
                return "malformed attribute"
            depth = 1
            index += 1
            while index < len(words) and depth:
                depth += (words[index] == "[") - (words[index] == "]")
                index += 1
            if depth:
                return "unterminated attribute"
            continue
        if words[index] == "pub":
            index += 1
            if index < len(words) and words[index] == "(":
                while index < len(words) and words[index] != ")":
                    index += 1
                index += 1
        if index >= len(words):
            return "incomplete item"
        kind = words[index]
        index += 1
        if kind == "mod":
            if index + 1 >= len(words) or words[index + 1] != ";":
                return "inline module or module implementation belongs in a feature file"
            index += 2
        elif kind in ("use", "extern"):
            while index < len(words) and words[index] != ";":
                index += 1
            if index >= len(words):
                return "unterminated import"
            index += 1
        else:
            return f"{kind} item belongs in its owning feature file"
    return None


def check(root=ROOT):
    failures = []
    roots = []
    for area in (root / "apps", root / "crates"):
        for path in sorted(area.rglob("*.rs")):
            if path.name == "mod.rs":
                failures.append(f"{path.relative_to(root)}: use feature.rs beside feature/")
            elif path.name == "lib.rs":
                roots.append(path)
                try:
                    error = composition_error(path.read_text(encoding="utf-8"))
                except ValueError as failure:
                    error = str(failure)
                if error:
                    failures.append(f"{path.relative_to(root)}: {error}")
    for failure in failures:
        print(failure, file=sys.stderr)
    if not failures:
        print(f"Checked modern module names and {len(roots)} composition-only crate roots.")
    return int(bool(failures))


if __name__ == "__main__":
    sys.exit(check())
