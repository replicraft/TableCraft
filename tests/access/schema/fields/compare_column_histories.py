"""Compare explicit deleted-declaration derivatives with inert independent getters.

These are original engineering derivatives, not Access-generated DROP COLUMN saves.
Generation preserves each original base and modifies only fresh, separate copies.
"""
import argparse
import hashlib
from pathlib import Path
import struct
from compare_browsing import PROFILES, RECIPES, display, run

PAGE_BYTES = 4096
COLUMN_BLOCK = 63
COLUMN_BYTES = 25
CURRENT_COUNT = 45
DEFINITION_LENGTH = 8


def remove_columns(data, page, dropped):
    start = page * PAGE_BYTES
    assert struct.unpack_from("<I", data, start + 51)[0] == 0, "indexed derivative unsupported"
    count = struct.unpack_from("<H", data, start + CURRENT_COUNT)[0]
    length = struct.unpack_from("<I", data, start + DEFINITION_LENGTH)[0]
    assert length <= PAGE_BYTES, "chained derivative unsupported"
    entries = [bytes(data[start + COLUMN_BLOCK + i * COLUMN_BYTES:start + COLUMN_BLOCK + (i + 1) * COLUMN_BYTES]) for i in range(count)]
    position = start + COLUMN_BLOCK + count * COLUMN_BYTES
    names = []
    for _ in entries:
        size = struct.unpack_from("<H", data, position)[0]
        names.append(bytes(data[position:position + 2 + size]))
        position += 2 + size
    keep = [i for i, name in enumerate(names) if name[2:].decode("utf-16-le") not in dropped]
    assert len(keep) == count - len(dropped)
    replacement = bytearray(data[start:start + COLUMN_BLOCK])
    replacement.extend(b"".join(entries[i] for i in keep))
    replacement.extend(b"".join(names[i] for i in keep))
    replacement.extend(data[position:start + length])
    struct.pack_into("<H", replacement, CURRENT_COUNT, len(keep))
    struct.pack_into("<I", replacement, DEFINITION_LENGTH, len(replacement))
    data[start:start + PAGE_BYTES] = replacement + bytes(PAGE_BYTES - len(replacement))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            base = args.directory / f"HistoryBase-{profile}.{suffix}"
            definitions = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "ColumnHistoryFixture.java", base, profile)
            data = bytearray(base.read_bytes())
            for line in definitions.splitlines():
                name, page = line.split("\t")
                dropped = {"DropFixed"} if name == "FixedGap" else {"DropText"} if name == "VariableGap" else {"DropFixed", "DropText"}
                remove_columns(data, int(page), dropped)
            (args.directory / f"HistoryGaps-{profile}.{suffix}").write_bytes(data)
    for profile, suffix in PROFILES:
        base = args.directory / f"HistoryBase-{profile}.{suffix}"
        path = args.directory / f"HistoryGaps-{profile}.{suffix}"
        before = sha(path)
        base_before = sha(base)
        for table in ["FixedGap", "VariableGap", "MixedGap", "AmbiguousGap"]:
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", path, table)
            actual = run(args.rows, path, table)
            assert actual == expected, (profile, table, "complete tagged rows", actual, expected)
            assert run(args.rows, path, table, "--batch-size", 2) == expected, (profile, table, "cursor")
            # Public CLI framing uses the normal escaped-display/csv boundary.
            header = ["ID", "DropText", "KeepText", "Flag", "Counter"] if table == "FixedGap" else ["ID", "DropFixed", "KeepText", "Flag", "Counter"] if table == "VariableGap" else ["ID", "KeepText", "Flag", "Counter"]
            wanted = "\t".join(header) + "\n" + "".join("\t".join(display(tag) for tag in line.split("\t")[1:]) + "\n" for line in expected.splitlines())
            assert run(args.cli, "rows", path, table, "--limit", 20) == wanted, (profile, table, "CLI")
            wanted_query = "ID\tCounter\n" + "".join(display(line.split("\t")[1]) + "\t" + display(line.split("\t")[-1]) + "\n" for line in expected.splitlines())
            assert run(args.cli, "query", "run", f"SELECT [ID],[Counter] FROM [{table}] ORDER BY [ID]", "--database", path) == wanted_query, (profile, table, "projected query")
        assert sha(path) == before and sha(base) == base_before
        print(f"{profile}: 16 full/cursor/CLI/query controls including known prefixes of ambiguous histories; unchanged original {base_before}; derivative {before}")


if __name__ == "__main__":
    main()
