"""Reproduce the candidate English error catalog from reviewed local references.

This tool performs no network access. VBA topic titles are CC-BY-4.0 data;
.NET numeric resource IDs only select the candidate runtime-code intersection.
"""

import argparse
import hashlib
import html
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate(table, runtime, topics):
    rows = {
        int(match[1]): match[3]
        for match in re.finditer(
            r"^\|(\d+)\|\[([^\]]+)\]\(([^)]+)\)\|", table.read_text(), re.M
        )
    }
    keys = {
        int(node.attrib["name"][2:])
        for node in ET.parse(runtime).getroot().findall("data")
        if re.fullmatch(r"ID\d+", node.attrib["name"])
    }
    messages = []
    sources = []
    for code in sorted(rows.keys() & keys):
        name = rows[code]
        if not re.fullmatch(r"[a-z0-9-]+\.md", name):
            raise ValueError("invalid topic path")
        source = topics / name
        raw = re.search(r"^title: (.+)$", source.read_text(), re.M)[1].strip()
        if raw.startswith('"') and raw.endswith('"'):
            raw = json.loads(raw)
        elif raw.startswith("'") and raw.endswith("'"):
            raw = raw[1:-1].replace("''", "'")
        message = re.sub(r" \(Error \d+\)(?: \[\d+ of \d+\])?$", "", html.unescape(raw))
        if not message or any(c in message for c in "{}\r\n"):
            raise ValueError("message is outside the flat Fluent profile")
        messages.append(f"error-{code} = {message}")
        sources.append({"code": code, "topic": name, "sha256": digest(source)})
    text = "\n".join([
        "# Diagnostic titles adapted from MicrosoftDocs/VBA-Docs (CC-BY-4.0).",
        "# Copyright Microsoft Corporation and contributors. See NOTICE and LICENSE.",
        "# Candidate English wording; not native Access Error() qualification.",
        "error-unknown = Application-defined or object-defined error",
        *messages,
        "",
    ])
    manifest = {
        "table_sha256": digest(table),
        "runtime_ids_sha256": digest(runtime),
        "catalog_sha256": hashlib.sha256(text.encode()).hexdigest(),
        "topics": sources,
    }
    return text, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--table", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--topics", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    text, manifest = generate(args.table, args.runtime, args.topics)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "en-US.ftl").write_text(text)
    (args.output / "provenance.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Generated {len(manifest['topics'])} candidate diagnostic messages.")


if __name__ == "__main__":
    main()
