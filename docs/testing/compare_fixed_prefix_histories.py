"""Compare visible fixed prefixes of deleted-column histories with raw getters.

Original engineering derivatives only; no Access DDL/save qualification is implied.
"""
import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import tempfile

from compare_browsing import PROFILES, RECIPES, display, run
from compare_column_histories import remove_columns
from compare_union_types import JAR_SHA256

PAGE_BYTES = 4096
INPUT_HASHES = {
    "V2000": ("47a5b2e2f726182e04688d581a0456d2d88ca8384736a89310cb5b3cdee275be", "e782bde7beabb3332f06a8e52ffabff2d073f58ae6e8f810daffdcd74d40f2a8"),
    "V2007": ("16c0a130cdc6c044fe22403916b928790e36e58e4b9eeec35cfcae6db4355611", "79f70241cfb1f16dd5059e6d4eb1928881127ac296f1c75eff0741a144fa28da"),
    "V2016": ("b70a5eb9a7708a4ec6768efd3e3c8832cf90d12486a3828b4cc1f51dc7171805", "ae9a426fc1b87a2db5cb51b40b76d27d9b749716eb37f79fa0723655046d0784"),
    "V2019": ("84f17e290524e8f46df06538e762f326104910049415e3cd5c9a0b8615fa841b", "f0b0ef7371da0ce5a1e2acaccb2e8c2d51a9f88be7fd1ca71abdc9c02f6b20d9"),
}


def record(data, definition, identifier):
    matches = []
    for base in range(0, len(data), PAGE_BYTES):
        page = data[base:base + PAGE_BYTES]
        if page[:2] != b"\x01\x01" or struct.unpack_from("<I", page, 4)[0] != definition:
            continue
        count = struct.unpack_from("<H", page, 12)[0]
        assert count <= 255
        for slot in range(count):
            raw = struct.unpack_from("<H", page, 14 + 2 * slot)[0]
            assert raw & 0xE000 == 0, "Controlled input unexpectedly relocated/deleted"
            start = raw & 0x1FFF
            end = PAGE_BYTES if slot == 0 else struct.unpack_from("<H", page, 12 + 2 * slot)[0] & 0x1FFF
            if end - start >= 6 and struct.unpack_from("<i", page, start + 2)[0] == identifier:
                matches.append((base, slot, base + start, base + end))
    assert len(matches) == 1, (definition, identifier, matches)
    return matches[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            base = args.directory / f"PrefixBase-{profile}.{suffix}"
            output = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                         RECIPES / "FixedPrefixHistoryFixture.java", base, profile)
            definitions = [(name, int(page)) for name, page in
                           (line.split("\t") for line in output.splitlines())]
            assert {name for name, _ in definitions} == {"FixedFirst", "VariableFirst"}
            data = bytearray(base.read_bytes())
            for _, page in definitions:
                remove_columns(data, page, {"DropFixed", "DropText"})
            (args.directory / f"PrefixGaps-{profile}.{suffix}").write_bytes(data)
            (args.directory / f"Definitions-{profile}.tsv").write_text(output)

    controls = 0
    for profile, suffix in PROFILES:
        base = args.directory / f"PrefixBase-{profile}.{suffix}"
        path = args.directory / f"PrefixGaps-{profile}.{suffix}"
        original = path.read_bytes()
        base_original = base.read_bytes()
        if not args.generate:
            assert (hashlib.sha256(base_original).hexdigest(), hashlib.sha256(original).hexdigest()) == INPUT_HASHES[profile]
        definitions = [(name, int(page)) for name, page in (line.split("\t") for line in
                       (args.directory / f"Definitions-{profile}.tsv").read_text().splitlines())]
        for table, definition in definitions:
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                           RECIPES / "TableDataOracle.java", path, table)
            assert run(args.rows, path, table) == expected
            assert run(args.rows, path, table, "--batch-size", 1) == expected
            rows = [line.split("\t")[1:] for line in expected.splitlines()]
            assert len(rows) == 4
            assert rows[:3] == [[f"I32:{i}", "N", "B0", "N"] for i in [1, 2, 3]]
            wanted = "ID\tKeepText\tFlag\tCounter\n" + "".join(
                "\t".join(display(cell) for cell in row) + "\n" for row in rows)
            assert run(args.cli, "rows", path, table) == wanted
            assert run(args.cli, "rows", path, table, "--stream") == wanted
            assert run(args.cli, "query", "run", f"SELECT * FROM [{table}] ORDER BY ID",
                       "--database", path) == wanted
            projected = "ID\tCounter\n" + "".join(display(row[0]) + "\t" + display(row[3]) + "\n" for row in rows)
            assert run(args.cli, "query", "run", f"SELECT [ID],[Counter] FROM [{table}] ORDER BY [ID]",
                       "--database", path) == projected
            assert run(args.cli, "query", "run", f"SELECT Count(*) AS N FROM [{table}]",
                       "--database", path) == "N\n4\n"
            controls += 7

            for damage in ["zero-count", "short-prefix", "required-tail"]:
                changed = bytearray(original)
                if damage == "required-tail":
                    _, _, start, end = record(changed, definition, 4)
                    assert struct.unpack_from("<H", changed, start)[0] == 6
                    struct.pack_into("<H", changed, end - 3, 0)
                else:
                    page, slot, start, end = record(changed, definition, 2)
                    assert struct.unpack_from("<H", changed, start)[0] == 2
                    if damage == "zero-count":
                        struct.pack_into("<H", changed, start, 0)
                    else:
                        struct.pack_into("<H", changed, page + 14 + 2 * slot, end - page - 6)
                        changed[end - 6:end] = b"\x02\0\x02\0\0\0"
                with tempfile.TemporaryDirectory(prefix="tablecraft-prefix-damage-") as temporary:
                    copy = Path(temporary) / path.name
                    copy.write_bytes(changed)
                    for command in [
                            ["rows", copy, table],
                            ["query", "run", f"SELECT ID FROM [{table}]", "--database", copy],
                            ["query", "run", f"SELECT Count(*) FROM [{table}]", "--database", copy]]:
                        result = subprocess.run([str(args.cli), *map(str, command)], capture_output=True)
                        assert result.returncode == 2 and not result.stdout, (profile, table, damage, result)
                        controls += 1
            assert path.read_bytes() == original and base.read_bytes() == base_original
        print(profile, len(base_original), hashlib.sha256(base_original).hexdigest(),
              len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f"Passed {controls} independent historical-prefix controls; native Access remains unqualified")


if __name__ == "__main__":
    main()
