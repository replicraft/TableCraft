"""Check declared workspace dependencies, including optional and target edges."""
import json
from pathlib import Path
import subprocess
import sys


# Explicit existing package contracts. A new crate must receive an ownership
# review here, rather than inheriting access to every other crate in its group.
ALLOWED = {
    "tablecraft-core": set(),
    "tablecraft-schema": {"tablecraft-core"},
    "tablecraft-model": {"tablecraft-core", "tablecraft-schema"},
    "tablecraft-io": {"tablecraft-core"},
    "tablecraft-db": {"tablecraft-core", "tablecraft-schema"},
    "tablecraft-sql": set(),
    "tablecraft-expressions": {"tablecraft-core", "tablecraft-sql"},
    "tablecraft-query": {"tablecraft-core", "tablecraft-schema", "tablecraft-db",
                         "tablecraft-sql", "tablecraft-expressions"},
    "tablecraft-ace": {"tablecraft-core", "tablecraft-schema", "tablecraft-model",
                       "tablecraft-io", "tablecraft-sql", "tablecraft-expressions",
                       "tablecraft-query"},
    "tablecraft-access": {"tablecraft-core", "tablecraft-schema", "tablecraft-model",
                          "tablecraft-db", "tablecraft-ace", "tablecraft-query",
                          "tablecraft-sql"},
    "tablecraft-sqlite": {"tablecraft-core", "tablecraft-schema", "tablecraft-db",
                          "tablecraft-query", "tablecraft-sql", "tablecraft-expressions"},
    "tablecraft-cli": {"tablecraft-core", "tablecraft-schema", "tablecraft-model",
                       "tablecraft-io", "tablecraft-db", "tablecraft-ace",
                       "tablecraft-access", "tablecraft-sqlite", "tablecraft-sql",
                       "tablecraft-expressions", "tablecraft-query"},
}


def violations(metadata):
    """Return policy failures over Cargo's resolved package names, not aliases."""
    members = set(metadata["workspace_members"])
    packages = {p["name"]: p for p in metadata["packages"] if p["id"] in members}
    paths = {Path(p["manifest_path"]).resolve().parent: p["name"]
             for p in packages.values()}
    root = Path(metadata["workspace_root"]).resolve()
    errors = []
    for name, package in sorted(packages.items()):
        if name not in ALLOWED:
            errors.append(f"{name}: package has no reviewed dependency policy")
            continue
        directory = Path(package["manifest_path"]).resolve().parent
        library_kinds = {"lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"}
        is_library = any(library_kinds.intersection(t["kind"]) for t in package["targets"])
        for dep in package["dependencies"]:
            target = dep["name"]
            # Cargo metadata exposes all kinds, optional edges and cfg targets.
            # A path dependency outside the workspace cannot bypass the policy.
            path = dep.get("path")
            if path:
                actual = paths.get(Path(path).resolve())
                if actual is None:
                    errors.append(f"{name} -> {target}: local dependency is outside reviewed workspace")
                    continue
                target = actual
            if target not in packages:
                continue
            target_directory = Path(packages[target]["manifest_path"]).resolve().parent
            if is_library and target_directory.is_relative_to(root / "apps"):
                errors.append(f"{name} -> {target}: library must not depend on an application")
            if target not in ALLOWED[name]:
                errors.append(f"{name} -> {target}: violates reviewed package boundary")
        if is_library and not directory.is_relative_to(root / "crates"):
            errors.append(f"{name}: library belongs under crates/")
    return sorted(set(errors))


def main():
    root = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline", "--locked"],
        cwd=root, capture_output=True, text=True, check=False,
    )
    if result.returncode:
        print(result.stderr, file=sys.stderr, end="")
        return result.returncode
    metadata = json.loads(result.stdout)
    errors = violations(metadata)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Dependency boundaries passed for {len(metadata['workspace_members'])} workspace packages.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
