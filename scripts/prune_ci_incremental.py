"""Remove only a completed native CI phase's incremental compiler cache.

The caller must have waited for Cargo and exclude concurrent cache/namespace
mutation. This is not a live-build cleanup or a general target-directory cleaner.
"""
import os
from pathlib import Path
import re
import shutil
import stat
import sys


ROOT = Path(__file__).resolve().parents[1]
TARGET_TRIPLE = re.compile(r"[a-z0-9_]+(?:-[a-z0-9_]+){2,}\Z")
CUSTOM_ROOT_VARIABLES = ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR")


def require_directory(path):
    information = path.lstat()
    reparse = getattr(information, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
    if stat.S_ISLNK(information.st_mode) or reparse or not stat.S_ISDIR(information.st_mode):
        raise ValueError("Incremental cache parents must be ordinary directories")


def prune(target, root=ROOT):
    if not isinstance(target, str) or not TARGET_TRIPLE.fullmatch(target):
        raise ValueError("One ordinary Rust target triple is required")
    require_directory(root)
    workspace = root.resolve(strict=True)
    cache = workspace
    for component in ("target", target, "debug", "incremental"):
        cache = cache / component
        try:
            require_directory(cache)
        except FileNotFoundError:
            return False
    cache.resolve(strict=True).relative_to(workspace)
    # rmtree unlinks contained symlinks rather than following them. Parents have
    # already been checked above; no compiler may be writing this cache now.
    shutil.rmtree(cache)
    return True


def main():
    try:
        if any(os.environ.get(name) for name in CUSTOM_ROOT_VARIABLES):
            raise ValueError("Custom Cargo cache roots are outside this utility's scope")
        target = os.environ.get("CARGO_BUILD_TARGET")
        removed = prune(target)
        action = "Removed completed" if removed else "No existing"
        print(f"{action} native incremental cache for {target}.")
        return 0
    except (OSError, ValueError) as error:
        print(f"Incremental cache prune refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
