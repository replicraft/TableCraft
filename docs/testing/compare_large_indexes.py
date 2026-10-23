"""Compare larger native indexed queries with independent read-only stored cursors."""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile
import uuid

from compare_guid_seeks import JAR_SHA256

RECIPES = Path(__file__).resolve().parent
FORMATS = {"jet4.mdb": "V2000", "2007.accdb": "V2007", "2016.accdb": "V2016", "2019.accdb": "V2019"}
INPUT_HASHES = {
    "jet4.mdb": "1789eff8f43b19dab43a1630be83ca472449d63c5a49a900104bea8e2fb05415",
    "2007.accdb": "40faf84fd101e57ea6f3c115c31fbaca5cc527054e56bec5f106d598c2812c7f",
    "2016.accdb": "cf842bae9c927e0515f5fb15e8b0318e4e5fccf3609396f28f557a6ba8170345",
    "2019.accdb": "58c1bba47f65bea61c2a2c6a06f0022eb05667d40ef22580dee0ecb6768a654a",
}
# Fresh local regeneration, 23 October 2026; retain the original identities above.
RESTORED_INPUT_HASHES = {
    "jet4.mdb": "4ca8dcdf0f66f3e6688db19573b1846c326f9c7416ad8f6caf3a687675f0018f",
    "2007.accdb": "7605dae8ff18fb13ee5a39f9e3f53ea97e65d829ec5642095a488787cdf491c3",
    "2016.accdb": "339d125b40367eaa11abd4ceff26ea69f5aab62bc9a82d76ba05436b274fbefe",
    "2019.accdb": "75a6a875a62a3a4e7362eaf47383541b47ef9198a9daa2fd2eae8ac530c123c4",
}
REGISTERED_INPUT_HASHES = {
    original: frozenset((original, RESTORED_INPUT_HASHES[suffix]))
    for suffix, original in INPUT_HASHES.items()
}
INDEXES = ("IdAsc", "IdDesc", "BucketAsc", "BucketDesc", "Pair", "ByteAsc", "ShortDesc",
           "BoolAsc", "MoneyAsc", "SingleAsc", "DoubleDesc", "StampAsc", "GuidAsc", "GuidDesc",
           "BinaryAsc", "TextAsc", "AmountDesc")


def registered_input_hash(path, original_hash):
    """Select a registered generation and retain its exact digest for this run."""
    if original_hash not in REGISTERED_INPUT_HASHES:
        raise ValueError("Unknown original fixture identity")

    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    assert actual in REGISTERED_INPUT_HASHES[original_hash], (path, actual)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true", help="Create fresh inputs; never overwrite")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0
    native = ["--native-bytes", "67108864", "--native-rows", "100000", "--native-steps", "10000000"]
    execution = ["--execution-steps", "10000000"]
    with tempfile.TemporaryDirectory(prefix="tablecraft-large-index-oracle-") as temporary:
        subprocess.run(["javac", "-cp", str(args.jar), "-d", temporary,
                        str(RECIPES / "LargeIndexFixture.java"), str(RECIPES / "IndexRowsOracle.java")], check=True)
        java = ["java", "-Dstdout.encoding=UTF-8", "-cp", os.pathsep.join((str(args.jar), temporary))]
        if args.generate:
            args.directory.mkdir(parents=True, exist_ok=True)
            for suffix, format_name in FORMATS.items():
                subprocess.run([*java, "LargeIndexFixture", str(args.directory / ("index-" + suffix)), format_name], check=True)
        for suffix, format_name in FORMATS.items():
            path = args.directory / ("index-" + suffix)
            original = path.read_bytes()
            digest = hashlib.sha256(original).hexdigest()
            if not args.generate:
                assert digest == registered_input_hash(path, INPUT_HASHES[suffix]), (path, digest)
            print(path.name, len(original), digest, flush=True)

            def cli(sql, index, options=None, accepted=True):
                if options is None:
                    options = native + execution
                result = subprocess.run([str(args.cli), "query", "run", sql, "--database", str(path),
                                         "--index", "IndexedRows", index, *options],
                                        capture_output=True, text=True, encoding="utf-8")
                assert result.returncode == (0 if accepted else 2), (path, index, sql, result.stderr)
                if not accepted:
                    assert not result.stdout, (path, index, sql, result.stdout[:100])
                    return None
                assert not result.stderr, result.stderr
                return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))

            large_number = format_name in ("V2016", "V2019")
            for index in (*INDEXES, *(("BigAsc",) if large_number else ())):
                output = subprocess.check_output([*java, "IndexRowsOracle", str(path), "IndexedRows", index,
                                                   "ID", "Bucket", "GuidValue"], text=True, encoding="utf-8")
                rows = []
                for line in output.splitlines():
                    identifier, bucket, guid = line.split("\t")
                    rows.append((int(identifier), None if bucket == "NULL" else int(bucket),
                                 None if guid == "NULL" else uuid.UUID(guid.strip("{}"))))
                identifiers = [row[0] for row in rows]
                assert sorted(identifiers) == list(range(1, 20001))
                assert all(bucket == (None if identifier % 5 == 0 else identifier % 3 - 1)
                           for identifier, bucket, _ in rows)
                assert cli("SELECT ID FROM IndexedRows", index) == [["ID"], *[[str(value)] for value in identifiers]], (path, index)
                assert cli("SELECT Count(*) AS Total FROM IndexedRows", index) == [["Total"], ["20000"]], (path, index)
                controls += 2
                # Request a second output field in addition to any hidden index key.
                projected = cli("SELECT TOP 3 ID,Bucket FROM IndexedRows", index)
                assert projected == [["ID", "Bucket"], *[[str(identifier), "NULL" if bucket is None else str(bucket)]
                                                          for identifier, bucket, _ in rows[:3]]], (path, index)
                controls += 1
                if index in ("IdAsc", "IdDesc"):
                    cases = [("ID=20000", lambda identifier, bucket, guid: identifier == 20000),
                             ("ID>=19998", lambda identifier, bucket, guid: identifier >= 19998),
                             ("ID=20001", lambda identifier, bucket, guid: False)]
                elif index in ("BucketAsc", "BucketDesc"):
                    cases = [("Bucket=1", lambda identifier, bucket, guid: bucket == 1),
                             ("Bucket IS NULL", lambda identifier, bucket, guid: bucket is None)]
                elif index in ("GuidAsc", "GuidDesc"):
                    key = uuid.UUID("6b29fc40-ca47-1067-b31d-00dd010662da")
                    cases = [("GuidValue={GUID {" + str(key) + "}}", lambda identifier, bucket, guid: guid == key),
                             ("GuidValue IS NULL", lambda identifier, bucket, guid: guid is None)]
                elif index == "BigAsc":
                    cases = [("BigValue>=9007199254740995", lambda identifier, bucket, guid: bucket == 1)]
                else:
                    cases = []
                for predicate, matches in cases:
                    expected = [["ID"], *[[str(identifier)] for identifier, bucket, guid in rows
                                           if matches(identifier, bucket, guid)]]
                    # Omitted controls must permit a small seek in a larger table.
                    assert cli("SELECT ID FROM IndexedRows WHERE " + predicate, index, options=[]) == expected, (path, index, predicate)
                    controls += 1
                print(path.name, index, "passed", flush=True)
            for sql, index, options in [
                ("SELECT ID FROM IndexedRows", "IdAsc", []),
                ("SELECT ID FROM IndexedRows", "MissingIndex", native + execution),
                ("SELECT ID FROM IndexedRows WHERE Bucket=1", "BucketAsc",
                 ["--native-rows", "5000", "--native-bytes", "67108864", *execution]),
                ("SELECT ID FROM IndexedRows WHERE ID=20000 AND 1/0=0", "IdAsc", native + execution),
            ]:
                cli(sql, index, options=options, accepted=False)
                controls += 1
            assert cli("SELECT (SELECT Count(*) FROM IndexedRows) AS Total,ID FROM IndexedRows WHERE ID=20000",
                       "IdAsc") == [["Total", "ID"], ["20000", "20000"]]
            controls += 1
            assert path.read_bytes() == original
    print(f"Passed {controls} independent large-index/full/projected/seek/cache/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
