"""Check every Cargo workspace target without a Windows-sized argument list."""
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
MAX_COMMAND_UTF16_UNITS = 16_000


def configuration(source, root):
    directory = source.parent
    while True:
        for name in (".rustfmt.toml", "rustfmt.toml"):
            candidate = directory / name
            if candidate.is_file():
                return str(candidate.relative_to(root))
        if directory == root:
            return None
        directory = directory.parent


def target_groups(metadata, root):
    """Retain real target roots, editions and nearest repository configuration."""
    members = set(metadata["workspace_members"])
    groups = {}
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        for target in package["targets"]:
            source = Path(target["src_path"])
            if not source.is_absolute():
                source = root / source
            relative = str(source.relative_to(root))
            key = (target["edition"], configuration(source, root))
            groups.setdefault(key, set()).add(relative)
    return groups


def command_units(command):
    # list2cmdline accounts for spaces, quotes and trailing backslashes using
    # Windows argument quoting. Leave substantial room below CreateProcess's
    # complete command limit for executable resolution and process machinery.
    return len(subprocess.list2cmdline(command).encode("utf-16-le")) // 2 + 1


def commands(groups, maximum=MAX_COMMAND_UTF16_UNITS):
    for (edition, config), sources in sorted(groups.items(), key=lambda item: str(item[0])):
        prefix = ["rustfmt", "--check", "--edition", edition]
        if config is not None:
            prefix.extend(["--config-path", config])
        batch = prefix.copy()
        for source in sorted(sources):
            if command_units([*prefix, source]) > maximum:
                raise ValueError("One formatter target exceeds the command budget")
            if command_units([*batch, source]) > maximum:
                yield batch
                batch = prefix.copy()
            batch.append(source)
        if len(batch) > len(prefix):
            yield batch


def check(root=ROOT):
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=root, check=True, capture_output=True, text=True, encoding="utf-8",
    )
    groups = target_groups(json.loads(metadata.stdout), root)
    if not groups:
        raise ValueError("Cargo reported no workspace formatter targets")
    # Materialize the bounded command plan before running any formatter. Normal
    # recursive module discovery remains enabled, matching Cargo's target roots.
    batches = list(commands(groups))
    for command in batches:
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode != 0:
            return result.returncode
    print(f"Checked {sum(map(len, groups.values()))} target roots in {len(batches)} bounded rustfmt batches.")
    return 0


def main():
    try:
        return check()
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, file=sys.stderr)
        return error.returncode
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Rust format check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
