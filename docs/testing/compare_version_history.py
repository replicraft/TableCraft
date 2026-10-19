"""Compare visible-Memo bindings, exact stored histories and corrupt copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from collection_cli import history_report, refused
from compare_composite_constraints import roots, run
from compare_complex_links import column_layout
from compare_attachment_columns import fields


def stored(args, path, table, memo):
    expected = run("java", "-Dfile.encoding=UTF-8", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        "docs/testing/VersionHistoryOracle.java", path, table, memo)
    history_report(args, path, table, memo, expected)
    actual = run(args.rows, path, table, "--version-history", memo)
    if actual != expected:
        raise RuntimeError("Exact stored history differs: " + table + "/" + memo)
    return actual


def records(line):
    cells = line.split("\t")
    if (len(cells) - 3) % 3:
        raise RuntimeError("Invalid stored-history tagged record shape")
    return [tuple(cells[i:i + 3]) for i in range(3, len(cells), 3)]


def rejected(args, path, table, memo):
    refused(args.cli, path, table, "--version-history", memo)
    result = subprocess.run([str(args.rows), str(path), table, "--version-history", memo],
        capture_output=True, text=True)
    if result.returncode == 0 or result.stdout:
        raise RuntimeError("Unqualified/corrupt history published output")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019", "complex2016", "complex2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--ace2007", type=Path)
    parser.add_argument("--complex2007", type=Path)
    args = parser.parse_args()
    if bool(args.ace2007) != bool(args.complex2007):
        parser.error("--ace2007 and --complex2007 must be supplied together")
    cases = [(args.ace2016, args.complex2016, 5), (args.ace2019, args.complex2019, 6)]
    if args.ace2007:
        cases.append((args.ace2007, args.complex2007, 2))
    for path, old, code in cases:
        original, old_original = path.read_bytes(), old.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Unexpected ACE profile")
        canonical = ""
        for table, memo, parents, versions in (("Histories", "Notes", 4, 16), ("OtherHistories", "Details", 2, 2)):
            actual = stored(args, path, table, memo)
            lines = actual.splitlines()
            if len(lines) != parents or sum(len(records(line)) for line in lines) != versions:
                raise RuntimeError("History parent/version count differs")
            api = run("java", "-Dfile.encoding=UTF-8", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/VersionHistoryOracle.java", path, table, memo, "--api-order").splitlines()
            if len(api) != len(lines):
                raise RuntimeError("Reference API parent count differs")
            for raw_line, api_line in zip(lines, api):
                if (raw_line.split("\t")[:3] != api_line.split("\t")[:3]
                        or sorted(records(raw_line)) != sorted(records(api_line))):
                    raise RuntimeError("Public history API membership differs from raw storage")
            if table == "Histories":
                expected_bits = ("0000000000000000", "8000000000000000", "0000000000000001", "8000000000000001",
                    "3fd0000000000000", "bfd0000000000000", "bff4000000000000", "bffc000000000000",
                    "3ff000000006df38", "bff000000006df38", "40e5f903f35ba6e7", "c124103480000000", "41469240ffffffff")
                first = records(lines[0])
                if [value[2] for value in first] != ["Y64:" + bits for bits in expected_bits]:
                    raise RuntimeError("Stored DATE precision/alias probes changed")
                if first[0][1] != "T" or first[1][1] != "T" + "é🦀\0line\r\n".encode().hex():
                    raise RuntimeError("Empty/Unicode/NUL version text changed")
                if records(lines[3]) != [("I32:16", "N", "N")]:
                    raise RuntimeError("NULL stored value/time changed")
                # Separate reference-only observation: public newest-first/tie-ID
                # order differs from the deliberately unsorted stored view.
                if [v[0] for v in records(lines[1])] != ["I32:14", "I32:15"] or [v[0] for v in records(api[1])] != ["I32:15", "I32:14"]:
                    raise RuntimeError("Independent ordering control is missing")
            canonical += "".join(memo + "\t" + line + "\n" for line in lines)
        if len(stored(args, old, "Collections", "Notes").splitlines()) != 3:
            raise RuntimeError("Original populated history control changed")
        for memo in ("Plain", "Details", "missing", "ID"):
            rejected(args, path, "Histories", memo)
        catalog = run(args.cli, "inspect", path, "--catalog", "--show-system").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        labels = catalog[start].split("\t")
        ids = {cells[labels.index("Name")]: int(cells[labels.index("Native ID")])
            for cells in (line.split("\t") for line in catalog[start + 1:])}
        declaration = run(args.cli, "inspect", path, "--table", "Histories", "--complex").splitlines()[2].split("\t")
        hidden, _, template, flat, foreign, identity, _ = declaration
        rejected(args, path, "Histories", hidden)
        owner = fields(original, ids["Histories"])
        conceptual = fields(original, ids[template])
        child = fields(original, ids[flat])
        slots = roots(original, ids[flat])
        cases = []
        foreign_offset, foreign_number, columns = column_layout(original, ids[flat], foreign)
        identity_offset, _, _ = column_layout(original, ids[flat], identity)
        changed = bytearray(original)
        changed[slots[-1][0] + 2 + foreign_offset:slots[-1][0] + 6 + foreign_offset] = (999).to_bytes(4, "little")
        cases.append(changed)
        changed = bytearray(original)
        changed[slots[-1][0] + 2 + identity_offset:slots[-1][0] + 6 + identity_offset] = original[slots[0][0] + 2 + identity_offset:slots[0][0] + 6 + identity_offset]
        cases.append(changed)
        changed = bytearray(original)
        changed[slots[-1][1] - (columns + 7) // 8 + foreign_number // 8] &= ~(1 << (foreign_number % 8))
        cases.append(changed)
        changed = bytearray(original); changed[owner[hidden][0] + 16] = 0; cases.append(changed)
        modified = next(name for name, entry in conceptual.items() if original[entry[0]] == 8)
        changed = bytearray(original); changed[conceptual[modified][0] + 16] = 0x10; cases.append(changed)
        changed = bytearray(original)
        changed[conceptual["Notes"][1]] = ord("X"); changed[child["Notes"][1]] = ord("X")
        cases.append(changed)
        with tempfile.TemporaryDirectory(prefix="tablecraft-version-history-") as directory:
            copy = Path(directory) / "copy.accdb"
            for changed in cases:
                copy.write_bytes(changed); rejected(args, copy, "Histories", "Notes")
            changed = bytearray(original)
            number = child[modified][2]
            changed[slots[-2][1] - (columns + 7) // 8 + number // 8] &= ~(1 << (number % 8))
            copy.write_bytes(changed)
            stored(args, copy, "Histories", "Notes")
        if path.read_bytes() != original or old.read_bytes() != old_original:
            raise RuntimeError("Original history/complex input changed")
        print(path.name, "parent collections", 6, "stored versions", 18, "corrupt copies", len(cases), "file",
            hashlib.sha256(original).hexdigest(), "history", hashlib.sha256(canonical.encode()).hexdigest())
    print("Exact visible-Memo bindings, CLI memberships/display, stored order/DATE bits/NULLs, separate reference ordering and buffered failures passed; native Access, history mutation and latest-value policy unrun.")


if __name__ == "__main__":
    main()
