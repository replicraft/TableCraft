#!/usr/bin/env python3
"""Inventory normal desktop dependencies and assemble explicitly reviewed notices.

Cargo reads are locked/offline. Inventory, source hashes and consolidated texts
are generated at packaging time. License alternatives, exceptional source pins
and asset attributions come from maintained policy, never inferred authorship.
"""
import argparse
import copy
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import urllib.parse
import urllib.request

import release_contract as contract

SCHEMA = 1
ROLES = {"license", "notice", "attribution"}
MAX_TEXT_BYTES = 16 * 1024 * 1024
MAX_POLICY_BYTES = 16 * 1024 * 1024
PACKAGE = re.compile(r"^([A-Za-z0-9_-]+) v([^\s]+)(?:\s.*)?$")
POLICY = contract.ROOT / "packaging/desktop/notice-policy.json"
CACHE = contract.ROOT / ".cache/desktop-license-sources"


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def fingerprint(inventory):
    return hashlib.sha256(canonical(inventory)).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path):
    contract.regular(path, MAX_POLICY_BYTES)
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique)


def cargo(arguments, root):
    result = subprocess.run(
        ["cargo", *arguments, "--locked", "--offline"],
        cwd=root, capture_output=True, text=True, encoding="utf-8", check=True,
    )
    return result.stdout


def collect(root, metadata=None, trees=None):
    root = root.resolve()
    if metadata is None:
        metadata = json.loads(cargo(["metadata", "--format-version", "1"], root))
    if trees is None:
        trees = {
            target: cargo([
                "tree", "-p", "tablecraft-desktop", "--target", target,
                "--edges", "normal,no-proc-macro", "--prefix", "none",
                "--format", "{p}|{f}",
            ], root)
            for target in contract.TARGETS
        }
    if set(trees) != set(contract.TARGETS):
        raise ValueError("Exactly the six native target feature trees are required")
    by_name = {}
    for package in metadata["packages"]:
        by_name.setdefault((package["name"], package["version"]), []).append(package)
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    locked = {
        (package["name"], package["version"], package.get("source")): package
        for package in lock["package"]
    }
    members = set(metadata["workspace_members"])
    inventory = {"schema": SCHEMA, "product": "tablecraft-desktop", "packages": [], "targets": {}}
    packages = {}
    locations = {}
    for target, text in sorted(trees.items()):
        closure = {}
        for line in text.splitlines():
            if not line.strip():
                continue
            if "|" not in line:
                raise ValueError("Feature-bearing cargo tree format {p}|{f} is required")
            identity, feature_text = line.split("|", 1)
            match = PACKAGE.fullmatch(identity.strip())
            if match is None:
                raise ValueError("Malformed Cargo package identity")
            key = match.groups()
            candidates = by_name.get(key, [])
            if len(candidates) != 1:
                raise ValueError(f"Missing or ambiguous metadata source: {key}")
            package = candidates[0]
            source = package.get("source")
            record = {"name": key[0], "version": key[1], "declared_license": package.get("license")}
            path = Path(package["manifest_path"]).resolve().parent
            if source is None:
                if package["id"] not in members:
                    raise ValueError("Unreviewed non-workspace path dependency")
                record["source"] = "workspace"
                manifest = Path(package["manifest_path"]).resolve().relative_to(root)
                record["manifest"] = str(manifest).replace("\\", "/")
            elif source.startswith("registry+"):
                entry = locked.get((*key, source))
                if not entry or not entry.get("checksum"):
                    raise ValueError("Registry dependency lacks a locked checksum")
                record.update(source=source, checksum=entry["checksum"])
            else:
                raise ValueError("Git or other dependency sources need an explicit inventory contract")
            if key in packages and packages[key] != record:
                raise ValueError("Inconsistent dependency identity across targets")
            packages[key] = record
            locations[key] = path
            # Cargo appends a duplicate marker to the final formatted field.
            feature_text = feature_text.removesuffix(" (*)").strip()
            features = sorted(set(filter(None, feature_text.split(","))))
            if any(not feature or feature != feature.strip() for feature in features):
                raise ValueError("Malformed Cargo feature list")
            if key in closure and closure[key] != features:
                raise ValueError("Inconsistent per-target resolved features")
            closure[key] = features
        version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
        if ("tablecraft-desktop", version) not in closure:
            raise ValueError("Feature tree does not contain the desktop root")
        inventory["targets"][target] = [
            {"name": name, "version": version, "features": features}
            for (name, version), features in sorted(closure.items())
        ]
    inventory["packages"] = [record for _, record in sorted(packages.items())]
    return inventory, locations


def relative_file(base, value):
    if not isinstance(value, str) or not value or "\\" in value:
        raise ValueError("Source path must be a relative slash-separated path")
    path = Path(value)
    if path.is_absolute() or any(part in ("", ".", "..") for part in value.split("/")):
        raise ValueError("Source path escapes its reviewed root")
    candidate = base / path
    if candidate.is_symlink() or not candidate.resolve().is_relative_to(base.resolve()):
        raise ValueError("Source symlink escapes its reviewed root")
    return contract.regular(candidate, MAX_TEXT_BYTES)


def source_text(source, base, upstream=False, repository=None, package_root=None):
    if source.get("role") not in ROLES:
        raise ValueError("Every source needs an explicit license/notice/attribution role")
    if "text" in source:
        if source["role"] != "attribution" or not source.get("derived_from"):
            raise ValueError("Inline text is only for parent-bound reviewed attribution")
        if not isinstance(source["text"], str):
            raise ValueError("Inline attribution must be UTF-8 text")
        data = source["text"].encode("utf-8")
        if len(data) > MAX_TEXT_BYTES:
            raise ValueError("Inline attribution exceeds its text limit")
    else:
        field = "cache_file" if upstream else "path"
        if source.get("root") == "repository" and not upstream:
            base = repository
        elif source.get("root") is not None:
            raise ValueError("Unsupported reviewed source root")
        if base is None:
            raise ValueError("Missing reviewed source root")
        path = relative_file(base, source.get(field))
        data = path.read_bytes()
    if len(data) != source.get("bytes") or hashlib.sha256(data).hexdigest() != source.get("sha256"):
        raise ValueError("Reviewed source size/hash mismatch")
    if upstream:
        https_url(source.get("url"))
    derived = source.get("derived_from")
    if derived is not None:
        if not isinstance(derived, dict):
            raise ValueError("Derived attribution needs one explicit binary source")
        origin = {"package": package_root, "repository": repository}.get(derived.get("root"))
        if origin is None:
            raise ValueError("Derived attribution has an unsupported binary root")
        binary = relative_file(origin, derived.get("path"))
        if binary.stat().st_size != derived.get("bytes") or contract.digest(binary) != derived.get("sha256"):
            raise ValueError("Reviewed attribution parent binary changed")
    extraction = source.get("extraction")
    if extraction is not None:
        if source["role"] != "attribution":
            raise ValueError("Only attribution headers may be extracted; grants remain complete")
        if not isinstance(extraction, dict) or set(extraction) != {"start_line", "end_line"}:
            raise ValueError("Attribution extraction needs an explicit inclusive line range")
        start, end = extraction["start_line"], extraction["end_line"]
        lines = data.splitlines(keepends=True)
        if type(start) is not int or type(end) is not int or not 1 <= start <= end <= len(lines):
            raise ValueError("Attribution extraction range is outside its reviewed source")
        data = b"".join(lines[start - 1:end])
    text = data.decode("utf-8")
    if not text.strip() or "\0" in text:
        raise ValueError("License/notice text is blank or contains NUL")
    return data, source["role"]


def reviewed(entry):
    if entry.get("review_status") not in ("reviewed", "generated"):
        raise ValueError("All license choices and sources require explicit review")
    choice = entry.get("selected_license")
    if not isinstance(choice, str) or not choice.strip():
        raise ValueError("An explicit reviewed license expression is required")
    if not isinstance(entry.get("sources"), list) or not entry["sources"]:
        raise ValueError("Complete reviewed source texts are required")
    additional = entry.get("additional_obligations", [])
    if not isinstance(additional, list) or any(
        not isinstance(value, str) or not value.strip() for value in additional
    ):
        raise ValueError("Additional obligations must be explicit reviewed labels")


def assemble(root, inventory, locations, policy, upstream, cache):
    digest = fingerprint(inventory)
    if policy.get("schema") != SCHEMA or upstream.get("schema") != SCHEMA:
        raise ValueError("Unsupported desktop notice policy schema")
    if any(document.get("review_status") not in ("reviewed", "generated") for document in (policy, upstream)):
        raise ValueError("Both complete notice policies require explicit review")
    if policy.get("reviewed_inventory_sha256") != digest:
        raise ValueError("Desktop dependency/feature inventory changed or is not reviewed")
    entries = {}
    for document, remote in ((policy, False), (upstream, True)):
        for entry in document.get("packages", []):
            key = (entry.get("name"), entry.get("version"))
            if key in entries:
                raise ValueError("Duplicate package policy authority")
            entries[key] = (entry, remote)
    actual = {
        (entry["name"], entry["version"]): entry
        for entry in inventory["packages"] if entry["source"] != "workspace"
    }
    own = [
        {key: entry[key] for key in ("name", "version", "declared_license")}
        for entry in inventory["packages"] if entry["source"] == "workspace"
    ]
    project = policy.get("project_license", {})
    if policy.get("project_packages") != own or any(
        entry["declared_license"] != project.get("declared_license") for entry in own
    ):
        raise ValueError("Project package/license authority differs from inventory")
    project_file = relative_file(root, project.get("path"))
    if project_file != root / "LICENSE":
        raise ValueError("Project license must be the separately shipped root LICENSE")
    if project_file.stat().st_size != project.get("bytes") or contract.digest(project_file) != project.get("sha256"):
        raise ValueError("Project license changed since review")
    bindings = policy.get("content_bindings")
    if not isinstance(bindings, list) or not bindings:
        raise ValueError("Compiled content bindings require explicit review")
    seen_bindings = set()
    binding_names = set()
    for binding in bindings:
        origin = binding.get("root")
        package = binding.get("package", {})
        key = (package.get("name"), package.get("version"))
        if origin == "repository":
            base = root
        elif origin == "package" and key in actual:
            base = locations[key]
        else:
            raise ValueError("Compiled content binding has an unknown owner")
        name = binding.get("name")
        if name is not None:
            if not isinstance(name, str) or not name.strip() or name in binding_names:
                raise ValueError("Compiled content binding names must be unique")
            binding_names.add(name)
        identity = (origin, key, binding.get("path"))
        if identity in seen_bindings:
            raise ValueError("Duplicate compiled content binding")
        seen_bindings.add(identity)
        path = relative_file(base, binding.get("path"))
        if path.stat().st_size != binding.get("bytes") or contract.digest(path) != binding.get("sha256"):
            raise ValueError("Reviewed compiled content changed")
    if set(entries) != set(actual):
        raise ValueError("Notice policy must cover exactly the reviewed runtime closure")

    texts = []
    text_ids = {}
    text_bytes = 0
    canonical_texts = set()
    index = []

    def retain(entry, base, remote):
        nonlocal text_bytes
        reviewed(entry)
        references = []
        for source in entry["sources"]:
            cached = "cache_file" in source
            locations_count = sum(key in source for key in ("path", "cache_file", "text"))
            if locations_count != 1:
                raise ValueError("Each reviewed source needs exactly one path, cache_file or inline text")
            if remote and not cached and "text" not in source:
                raise ValueError("Upstream package policy requires pinned cached sources")
            data, role = source_text(source, cache if cached else base, cached, root, base)
            if data not in text_ids:
                text_ids[data] = len(texts) + 1
                texts.append(data)
                text_bytes += len(data)
                if text_bytes > contract.MAX_LEGAL_BYTES:
                    raise ValueError("Consolidated notices exceed the release text limit")
            if source.get("canonical") is True:
                if role != "license":
                    raise ValueError("Canonical terms cannot stand in for attribution")
                canonical_texts.add(text_ids[data])
            elif source.get("canonical") not in (None, False):
                raise ValueError("Canonical source marker must be boolean")
            references.append(f"{role} text {text_ids[data]}")
        result = ", ".join(references)
        if entry.get("additional_obligations"):
            result += " | additional obligations: " + ", ".join(entry["additional_obligations"])
        return result

    for key, item in sorted(actual.items()):
        entry, remote = entries[key]
        if entry.get("declared_license") != item["declared_license"]:
            raise ValueError("Declared license changed since policy review")
        base = locations[key]
        references = retain(entry, base, remote)
        index.append(f"{key[0]} {key[1]} | {entry['selected_license']} | {references}")
    assets = set()
    for entry in policy.get("assets", []):
        name = entry.get("name")
        if not isinstance(name, str) or not name.strip() or name in assets:
            raise ValueError("Asset notice names must be unique and explicit")
        assets.add(name)
        references = retain(entry, root, False)
        index.append(f"Asset: {name} | {entry['selected_license']} | {references}")
    if not assets:
        raise ValueError("Compiled asset/data notices must be explicitly reviewed")
    header = (
        "Third-party licenses for TableCraft desktop\n\n"
        f"Generated inventory SHA-256: {digest}\n"
        "This bundle covers the six-target normal dependency/feature closure\n"
        "and explicitly listed compiled assets. Build tools and proc macros are excluded.\n"
        "License choices and exceptional attribution sources were supplied by maintained policy;\n"
        "this generator does not choose alternatives or establish legal completeness.\n\n"
        "Component index\n===============\n\n" + "\n".join(index) + "\n"
    ).encode()
    sections = [header]
    for number, data in enumerate(texts, 1):
        sections.append(f"\nText {number}\n{'=' * (5 + len(str(number)))}\n\n".encode())
        if number in canonical_texts:
            sections.append(
                b"Canonical license terms; placeholder is unmodified, not package attribution.\n\n"
            )
        sections.append(data)
        if not data.endswith(b"\n"):
            sections.append(b"\n")
    output = b"".join(sections)
    if len(output) > contract.MAX_LEGAL_BYTES:
        raise ValueError("Consolidated notices exceed the release text limit")
    return output


def https_url(url):
    if not isinstance(url, str):
        raise ValueError("A reviewed HTTPS source URL is required")
    parts = urllib.parse.urlsplit(url)
    if (
        parts.scheme != "https" or not parts.hostname
        or parts.username is not None or parts.password is not None
        or parts.fragment or any(ord(character) < 32 for character in url)
    ):
        raise ValueError("Source URLs must be HTTPS without credentials or fragments")
    return parts


class SameOriginRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, newurl):
        old = https_url(request.full_url)
        new = https_url(newurl)
        if (old.hostname, old.port) != (new.hostname, new.port):
            raise ValueError("License source redirect left its reviewed HTTPS origin")
        return super().redirect_request(request, file, code, message, headers, newurl)


def cached_sources(documents):
    sources = {}
    names = {}
    for document in documents:
        if document.get("schema") != SCHEMA:
            raise ValueError("Unsupported desktop notice policy schema")
        if document.get("review_status") not in ("reviewed", "generated"):
            raise ValueError("Complete source policy requires explicit review")
        for entry in document.get("packages", []) + document.get("assets", []):
            remote = [source for source in entry.get("sources", []) if "cache_file" in source]
            if remote:
                reviewed(entry)
            for source in remote:
                https_url(source.get("url"))
                name = source["cache_file"]
                # Reuse source-path validation, permitting only canonical names.
                if not isinstance(name, str) or "\\" in name or Path(name).is_absolute():
                    raise ValueError("Cache filenames must be relative")
                if any(part in ("", ".", "..") for part in name.split("/")):
                    raise ValueError("Cache filenames must be canonical relative paths")
                portable = name.casefold()
                if portable in names and names[portable] != name:
                    raise ValueError("Cache paths differ only by case")
                names[portable] = name
                size = source.get("bytes")
                sha = source.get("sha256")
                if type(size) is not int or not 0 < size <= MAX_TEXT_BYTES:
                    raise ValueError("Cached source has an invalid reviewed size")
                if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{64}", sha):
                    raise ValueError("Cached source requires a reviewed SHA-256")
                identity = (source["url"], size, sha)
                if name in sources and sources[name] != identity:
                    raise ValueError("Conflicting reviewed cache-file sources")
                sources[name] = identity
    return sources


def fetch_sources(documents, cache, opener=None):
    sources = cached_sources(documents)
    if cache.is_symlink():
        raise ValueError("License cache root must not be a symlink")
    cache.mkdir(parents=True, exist_ok=True)
    if opener is None:
        opener = urllib.request.build_opener(SameOriginRedirect())
    for name, (url, size, sha) in sorted(sources.items()):
        destination = cache / name
        if not destination.resolve().is_relative_to(cache.resolve()):
            raise ValueError("License cache destination escaped its root")
        if destination.exists() or destination.is_symlink():
            path = contract.regular(destination, MAX_TEXT_BYTES)
            if path.stat().st_size != size or contract.digest(path) != sha:
                raise ValueError("Existing cached license source differs from reviewed input")
            continue
        request = urllib.request.Request(url, headers={"User-Agent": "TableCraft-license-audit"})
        with opener.open(request, timeout=30) as response:
            if response.geturl() != url:
                initial = https_url(url)
                final = https_url(response.geturl())
                if (initial.hostname, initial.port) != (final.hostname, final.port):
                    raise ValueError("License response left its reviewed HTTPS origin")
            body = response.read(size + 1)
        if len(body) != size or hashlib.sha256(body).hexdigest() != sha:
            raise ValueError("Downloaded license source size/hash differs from review")
        destination.parent.mkdir(parents=True, exist_ok=True)
        write_new(destination, body)


def load_inventory(root, args):
    metadata = read_json(args.metadata) if args.metadata else None
    trees = None
    if args.tree:
        trees = {}
        for value in args.tree:
            target, separator, path = value.partition("=")
            if not separator or target in trees:
                raise ValueError("Tree caches require unique TARGET=PATH entries")
            trees[target] = Path(path).read_text(encoding="utf-8")
    return collect(root, metadata, trees)


def write_new(path, content):
    created = False
    try:
        with path.open("xb") as output:
            created = True
            output.write(content)
    except BaseException:
        if created:
            path.unlink(missing_ok=True)
        raise


def bind_source(source, base):
    """Compute local source facts; remote pins and derived attributions stay exact."""
    result = copy.deepcopy(source)
    if "path" in result:
        path = relative_file(base, result["path"])
        result.update(bytes=path.stat().st_size, sha256=contract.digest(path))
    return result


def published_sources(base, selected, declared):
    """Retain published license/notice documents without interpreting their text."""
    sources = []
    selected_found = False
    names = {
        "MIT": {"license-mit", "license-mit.txt", "license-mit.md"},
        "Apache-2.0": {"license-apache", "license-apache.txt", "license-apache.md", "license-apache-2.0.txt"},
    }.get(selected, set())
    for path in sorted(base.rglob("*")):
        lower = path.name.lower()
        license_file = lower.startswith(("license", "licence", "copying"))
        notice = lower.startswith(("notice", "copyright"))
        if not (license_file or notice):
            continue
        if path.is_dir():
            continue
        source = {"path": path.relative_to(base).as_posix(), "role": "license" if license_file else "notice"}
        source = bind_source(source, base)
        # Refuse unsupported encodings/binary/empty documents before retaining.
        text = path.read_bytes().decode("utf-8")
        if not text.strip() or "\0" in text:
            raise ValueError("Published license/notice source is blank or binary")
        sources.append(source)
        if license_file and (lower in names or selected == declared):
            selected_found = True
    if not selected_found:
        raise ValueError(f"Selected license needs an explicit source exception: {selected}")
    return sources


def expand_policy(root, inventory, locations, policy):
    """Resolve compact human decisions into a disposable source manifest."""
    if policy.get("schema") != SCHEMA:
        raise ValueError("Unsupported desktop notice policy schema")
    exceptions = {}
    for entry in policy.get("packages", []):
        key = (entry["name"], entry["version"])
        if key in exceptions:
            raise ValueError("Duplicate package policy authority")
        exceptions[key] = entry
    generated = {
        "schema": SCHEMA, "review_status": "generated",
        "reviewed_inventory_sha256": fingerprint(inventory),
        "packages": [], "assets": [], "content_bindings": [],
        "project_packages": [],
    }
    for item in inventory["packages"]:
        key = (item["name"], item["version"])
        if item["source"] == "workspace":
            generated["project_packages"].append({
                field: item[field] for field in ("name", "version", "declared_license")
            })
            continue
        base = locations[key]
        if key in exceptions:
            entry = copy.deepcopy(exceptions[key])
            if entry["declared_license"] != item["declared_license"]:
                raise ValueError("Exception declared license differs from Cargo metadata")
            sources = []
            for source in entry["sources"]:
                if set(source) == {"source"}:
                    source = policy["sources"][source["source"]]
                sources.append(bind_source(source, base))
            entry["sources"] = sources
        else:
            declared = item["declared_license"]
            selected = policy.get("license_choices", {}).get(declared)
            if not selected:
                raise ValueError(f"Declared license needs explicit choice policy: {declared}")
            entry = {
                "name": key[0], "version": key[1], "declared_license": declared,
                "selected_license": selected,
                "sources": published_sources(base, selected, declared),
            }
        entry["review_status"] = "generated"
        generated["packages"].append(entry)
    # Stale version-specific exceptions are inert; no source is fetched for them.
    for entry in policy.get("assets", []):
        entry = copy.deepcopy(entry)
        entry["review_status"] = "generated"
        entry["sources"] = [bind_source(source, root) for source in entry["sources"]]
        generated["assets"].append(entry)
    for binding in policy.get("content_bindings", []):
        owner = binding.get("package", {})
        key = (owner.get("name"), owner.get("version"))
        base = root if binding["root"] == "repository" else locations.get(key)
        if base is None:
            continue
        generated["content_bindings"].append(bind_source(binding, base))
    project_license = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["license"]
    generated["project_license"] = bind_source({"path": "LICENSE", "declared_license": project_license}, root)
    return generated


def generate(root, policy, cache):
    """Generate fresh notices from locked Cargo sources and maintained policy.

    Missing upstream text must be fetched explicitly before packaging. Network
    access and policy decisions are never implicit packaging side effects.
    """
    inventory, locations = collect(root)
    expanded = expand_policy(root, inventory, locations, read_json(policy))
    content = assemble(root, inventory, locations, expanded,
                       {"schema": SCHEMA, "review_status": "generated", "packages": []}, cache)
    return inventory, expanded, content


@contextmanager
def bundle(path=None, policy=POLICY, cache=CACHE):
    """Own generated intermediate text for the duration of an archive operation."""
    if path is not None:
        yield path
        return
    _, _, content = generate(contract.ROOT, policy, cache)
    with tempfile.TemporaryDirectory(prefix="tablecraft-desktop-notices-") as directory:
        generated = Path(directory) / "ThirdPartyLicenses.txt"
        write_new(generated, content)
        yield generated


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("inventory", "sources", "generate", "fetch"))
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--tree", action="append", help="Feature-bearing TARGET=PATH cache; supply all six")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--policy", type=Path, default=POLICY)
    parser.add_argument("--cache", type=Path, default=CACHE)
    args = parser.parse_args()
    try:
        if args.command == "fetch":
            inventory, locations = collect(contract.ROOT)
            expanded = expand_policy(contract.ROOT, inventory, locations, read_json(args.policy))
            fetch_sources([expanded], args.cache)
            return
        if args.output is None:
            raise ValueError("Inventory/generate requires an explicit output path")
        if args.command in ("generate", "sources") and (args.metadata or args.tree):
            raise ValueError("Release notice generation requires a fresh locked/offline Cargo inventory")
        inventory, locations = load_inventory(contract.ROOT, args)
        if args.command == "inventory":
            content = (json.dumps(inventory, indent=2, sort_keys=True) + "\n").encode()
            print(fingerprint(inventory))
        else:
            expanded = expand_policy(contract.ROOT, inventory, locations, read_json(args.policy))
            if args.command == "sources":
                content = (json.dumps(expanded, indent=2, sort_keys=True) + "\n").encode()
            else:
                content = assemble(contract.ROOT, inventory, locations, expanded,
                                   {"schema": SCHEMA, "review_status": "generated", "packages": []}, args.cache)
        write_new(args.output, content)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Desktop notices refused: {error}\n")


if __name__ == "__main__":
    main()
