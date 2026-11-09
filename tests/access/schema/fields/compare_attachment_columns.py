"""Compare stored attachment metadata and reject corrupt disposable copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from collection_cli import attachment_report, refused, refused_page
from compare_composite_constraints import roots, run
from compare_complex_links import column_layout
from compare_decimal_relationships import variable_start


def fields(data, definition):
    start = definition * 4096
    count = int.from_bytes(data[start + 45:start + 47], "little")
    indexes = int.from_bytes(data[start + 51:start + 55], "little")
    block = start + 63 + indexes * 12
    names = block + count * 25
    result = {}
    for ordinal in range(count):
        size = int.from_bytes(data[names:names + 2], "little")
        name = data[names + 2:names + 2 + size].decode("utf-16-le")
        at = block + ordinal * 25
        result[name] = (at, names + 2, int.from_bytes(data[at + 5:at + 7], "little"),
            int.from_bytes(data[at + 7:at + 9], "little"))
        names += size + 2
    return result


def rejected(example, path, metadata):
    result = subprocess.run([str(example), str(path)] + (["--metadata"] if metadata else []),
        capture_output=True, text=True)
    if result.returncode == 0 or result.stdout:
        raise RuntimeError("Unqualified/corrupt attachment published output")


def payload_end(data, definition, encoded):
    tagged = int.from_bytes(data[definition:definition + 4], "little")
    if tagged & 0x3fffffff != len(encoded):
        raise RuntimeError("Expected exact encoded payload length")
    if tagged >> 30 == 2:
        start, end = definition + 12, definition + 12 + len(encoded)
    elif tagged >> 30 == 1:
        reference = int.from_bytes(data[definition + 4:definition + 8], "little")
        page, slot = (reference >> 8) * 4096, reference & 255
        if data[page:page + 2] != b"\x01\x01" or data[page + 4:page + 8] != b"LVAL":
            raise RuntimeError("Expected LVAL payload page")
        raw = int.from_bytes(data[page + 14 + slot * 2:page + 16 + slot * 2], "little")
        if raw & 0xe000:
            raise RuntimeError("Expected live direct LVAL row")
        start = page + raw
        end = page + (4096 if slot == 0 else int.from_bytes(
            data[page + 12 + slot * 2:page + 14 + slot * 2], "little") & 0x1fff)
    else:
        raise RuntimeError("Expected inline or single-row fixture payload")
    if data[start:end] != encoded:
        raise RuntimeError("Payload mutation does not bind exact independent metadata bytes")
    return end


def compare(args, path):
    expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        "docs/testing/AttachmentFixture.java", "--metadata", path)
    attachment_report(args, path, expected)
    actual = run(args.example, path, "--metadata")
    if actual != expected or len(actual.splitlines()) != 6:
        raise RuntimeError("Exact attachment metadata or encoded bytes differ")
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "example", "ace2016", "ace2019", "complex2016", "complex2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--ace2007", type=Path)
    parser.add_argument("--complex2007", type=Path)
    args = parser.parse_args()
    if bool(args.ace2007) != bool(args.complex2007):
        parser.error("--ace2007 and --complex2007 must be supplied together")
    names = ("FileURL", "FileName", "FileType", "FileData", "FileTimeStamp", "FileFlags")
    cases = [(args.ace2016, args.complex2016, 5), (args.ace2019, args.complex2019, 6)]
    if args.ace2007:
        cases.append((args.ace2007, args.complex2007, 2))
    for path, complex_path, code in cases:
        original = path.read_bytes()
        assert original[20:24] == bytes([code, 0, 0, 0])
        other_original = complex_path.read_bytes()
        actual = compare(args, path)
        catalog = run(args.cli, "inspect", path, "--catalog", "--show-system").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        labels = catalog[start].split("\t")
        ids = {cells[labels.index("Name")]: int(cells[labels.index("Native ID")])
            for cells in (line.split("\t") for line in catalog[start + 1:])}
        declaration = run(args.cli, "inspect", path, "--table", "Collections", "--complex").splitlines()[2].split("\t")
        _, _, template, flat, foreign, _, _ = declaration
        owner = ids[flat]
        layout = fields(original, owner)
        conceptual = fields(original, ids[template])
        slots = roots(original, owner)
        max_columns = int.from_bytes(original[owner * 4096 + 41:owner * 4096 + 43], "little")
        mask_bytes = (max_columns + 7) // 8
        cases = []
        changed = bytearray(original)
        for name in names:
            number = layout[name][2]
            changed[slots[0][1] - mask_bytes + number // 8] &= ~(1 << (number % 8))
        cases.append((changed, True))
        changed = bytearray(original)
        number = layout["FileData"][2]
        changed[slots[-1][1] - mask_bytes + number // 8] &= ~(1 << (number % 8))
        cases.append((changed, True))
        changed = bytearray(original)
        at = variable_start(original, slots[-1], layout["FileData"][3], max_columns)
        encoded = bytes.fromhex(actual.splitlines()[-1].split("\t")[5][1:])
        if encoded[:4] != bytes([1, 0, 0, 0]):
            raise RuntimeError("Expected compressed final fixture payload")
        changed[payload_end(original, at, encoded) - 1] ^= 1
        cases.append((changed, True))
        changed = bytearray(original)
        changed[conceptual["FileURL"][0] + 16] = 0x11
        cases.append((changed, False))
        changed = bytearray(original)
        changed[conceptual["FileTimeStamp"][1]] = ord("X")
        cases.append((changed, False))
        changed = bytearray(original)
        offset, _, _ = column_layout(original, owner, foreign)
        changed[slots[-1][0] + 2 + offset:slots[-1][0] + 6 + offset] = (999).to_bytes(4, "little")
        cases.append((changed, False))
        with tempfile.TemporaryDirectory(prefix="tablecraft-attachment-columns-") as directory:
            copy = Path(directory) / "copy.accdb"
            for changed, metadata_passes in cases:
                copy.write_bytes(changed)
                if metadata_passes:
                    compare(args, copy)
                else:
                    rejected(args.example, copy, True)
                    refused(args.cli, copy, "Collections", "--attachments", "Files")
                    refused_page(args.cli, copy, "Collections")
                rejected(args.example, copy, False)
        declarations = run(args.cli, "inspect", complex_path, "--table", "Collections", "--complex").splitlines()[2:]
        history = [row.split("\t")[0] for row in declarations if row.split("\t")[0] not in ("Choices", "Files")]
        if len(history) != 1:
            raise RuntimeError("Expected one resolved hidden history column")
        for column in ("Choices", "Notes", history[0]):
            refused(args.cli, complex_path, "Collections", "--attachments", column)
            result = subprocess.run([str(args.example), str(complex_path), "--column", column, "--metadata"],
                capture_output=True, text=True)
            if result.returncode == 0 or result.stdout:
                raise RuntimeError("Non-attachment collection was silently reinterpreted")
        if path.read_bytes() != original or complex_path.read_bytes() != other_original:
            raise RuntimeError("Original attachment/complex input changed")
        print(path.name, "attachments", 6, "disposable copies", len(cases), "file",
            hashlib.sha256(original).hexdigest(), "metadata", hashlib.sha256(actual.encode()).hexdigest())
    print("Exact metadata/encoded bytes, CLI memberships/display, NULL preservation, explicit late decode failures and strict template/type/reference rejection passed; native Access and mutation unrun.")


if __name__ == "__main__":
    main()
