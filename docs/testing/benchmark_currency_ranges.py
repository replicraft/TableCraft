"""Compare identical release queries through sequential and Currency-index reads."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile

from compare_currency_ranges import MONEY_HASHES
from compare_large_indexes import INPUT_HASHES, registered_input_hash


def public_results(report, replacements):
    allowed = {"command", "name", "measurements", "summary", "mean", "stddev",
               "median", "user", "system", "min", "max", "times", "exit_codes"}
    results = []
    for result in report["results"]:
        row = {key: value for key, value in result.items() if key in allowed}
        for original, label in replacements.items():
            row["command"] = row["command"].replace(original, label)
        results.append(row)
    return results


def wall(result):
    if "summary" in result:
        return result["summary"]["time_wall_clock"]
    return {"mean": result["mean"], "stddev": result["stddev"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "large", "small", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--excluded", type=Path)
    args = parser.parse_args()
    hashes = {
        args.large: registered_input_hash(args.large, INPUT_HASHES["2016.accdb"]),
        args.small: MONEY_HASHES["2016"],
    }
    for path, digest in hashes.items():
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest
    replacements = {str(args.cli): "${CLI}", str(args.large): "${LARGE_DB}", str(args.small): "${SMALL_DB}"}
    report = {
        "scope": "Release CLI process wall time, identical SQL, warm OS caches; independent/native Access qualification remains separate.",
        "platform": platform.platform(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(args.cli.read_bytes()).hexdigest(),
        "inputs": {replacements[str(path)]: digest for path, digest in hashes.items()},
        "runs": 15,
        "warmup": 3,
        "cases": [],
        "excluded_runs": [],
    }
    if args.excluded:
        excluded = json.loads(args.excluded.read_text())
        report["excluded_runs"].append({
            "reason": "Initial large-point timing overlapped validation/compilation and is excluded from performance conclusions.",
            "results": public_results(excluded, replacements),
        })
    cases = [
        ("large-point", args.large, "IndexedRows", "MoneyAsc",
         "SELECT Count(*) AS N FROM IndexedRows WHERE [Money]=CCur(0)", []),
        ("large-empty", args.large, "IndexedRows", "MoneyAsc",
         "PARAMETERS P CURRENCY; SELECT Count(*) AS N FROM IndexedRows WHERE [Money]=P",
         ["--param", "P=currency:922337203685477.5807"]),
        ("small-point", args.small, "MoneyIndexed", "MoneyDesc",
         "PARAMETERS P CURRENCY; SELECT Count(*) AS N FROM MoneyIndexed WHERE Amount=P",
         ["--param", "P=currency:900719925474.0993"]),
        ("small-nonnull", args.small, "MoneyIndexed", "MoneyAsc",
         "SELECT Count(*) AS N FROM MoneyIndexed WHERE Amount IS NOT NULL", []),
    ]
    with tempfile.TemporaryDirectory(prefix="tablecraft-currency-timing-") as temporary:
        for label, path, table, index, sql, params in cases:
            base = [str(args.cli), "query", "run", sql, "--database", str(path),
                    "--native-bytes", "67108864", "--native-rows", "100000",
                    "--native-steps", "10000000", "--execution-steps", "10000000", *params]
            indexed = [*base, "--index", table, index]
            expected = subprocess.check_output(base)
            assert expected == subprocess.check_output(indexed)
            output = Path(temporary) / (label + ".json")
            subprocess.run([
                "hyperfine", "--warmup", "3", "--runs", "15", "--export-json", str(output),
                "--command-name", "sequential", shlex.join(base),
                "--command-name", "currency-index", shlex.join(indexed),
            ], check=True, stdout=subprocess.DEVNULL)
            results = public_results(json.loads(output.read_text()), replacements)
            report["cases"].append({"case": label, "stdout": expected.decode(), "results": results})
            print(label, "mean_ms", [round(wall(row)["mean"] * 1000, 3) for row in results], flush=True)
    for path, digest in hashes.items():
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest
    assert hashlib.sha256(args.cli.read_bytes()).hexdigest() == report["binary_sha256"]
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print("Outputs agree; original inputs and benchmark binary remain unchanged.")


if __name__ == "__main__":
    main()
