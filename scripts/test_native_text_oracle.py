"""Evidence-integrity regressions; synthetic reports are not native ACE results."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "docs/testing/native_text_oracle.py"
SPEC = importlib.util.spec_from_file_location("native_text_oracle", SOURCE)
oracle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(oracle)


class NativeTextEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / "probes"
        oracle.prepare(self.directory)
        capture = self.directory / "capture"
        capture.mkdir()
        # A deliberately synthetic envelope for integrity checks, not an ACCDB.
        raw = bytearray(4096)
        raw[4:19] = b"Standard ACE DB"
        self.report = {
            "format": oracle.FORMAT,
            "request_sha256": hashlib.sha256(oracle.request_bytes()).hexdigest(),
            "runner_sha256": hashlib.sha256(SOURCE.with_name("Capture-NativeText.ps1").read_bytes()).hexdigest(),
            "environment": {"engine_version": "synthetic", "engine_dll_version": "synthetic",
                            "windows_version": "synthetic", "powershell_version": "synthetic", "process_bits": 64},
            "databases": [],
        }
        for phase in ["plain", "indexed"]:
            (capture / (phase + ".accdb")).write_bytes(raw)
            cases = []
            for case in oracle.request()["cases"]:
                if case["id"].endswith(".stored"):
                    table = next(t for t in oracle.request()["tables"] if case["id"].startswith(t["name"] + "."))
                    rows = [[{"kind": "integer", "value": str(i)}, {"kind": "null"} if value is None else {"kind": "text", "value": value}] for i, value in enumerate(table["values"])]
                    cases.append({"id": case["id"], "status": "ok", "columns": ["ID", "V"], "rows": rows})
                else:
                    cases.append({"id": case["id"], "status": "error", "hresult": -2147467259, "dao_numbers": [3071]})
            self.report["databases"].append({"id": phase, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                "header_hex": raw[:128].hex(), "database_collating_order": 1033,
                "field_collating_orders": {"ShortValues": 1033, "LongValues": 1033}, "results": cases})

    def check(self):
        (self.directory / "capture/results.json").write_text(json.dumps(self.report), encoding="utf-8")
        return oracle.validate(self.directory)

    def test_errors_remain_errors_not_passes(self):
        counts = self.check()
        self.assertEqual(counts["captured"], 4)
        self.assertEqual(counts["errors"], 2 * len(oracle.request()["cases"]) - 4)

    def test_refuses_overwriting_prepared_work(self):
        with self.assertRaises(FileExistsError):
            oracle.prepare(self.directory)

    def test_windows_runner_accepts_only_this_recipe(self):
        runner = SOURCE.with_name("Capture-NativeText.ps1").read_text(encoding="utf-8")
        digest = hashlib.sha256(oracle.request_bytes()).hexdigest()
        self.assertIn("$recipeSha256 = '" + digest + "'", runner)

    def test_changed_request_and_hash_fail(self):
        self.report["request_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "request hash"):
            self.check()
        (self.directory / "request.json").write_bytes(oracle.request_bytes() + b" ")
        with self.assertRaisesRegex(ValueError, "recipe"):
            self.check()

    def test_missing_build_and_phases_fail(self):
        del self.report["environment"]["engine_dll_version"]
        with self.assertRaisesRegex(ValueError, "environment"):
            self.check()
        self.report["environment"]["engine_dll_version"] = "synthetic"
        self.report["databases"].pop()
        with self.assertRaisesRegex(ValueError, "phase"):
            self.check()

    def test_changed_runner_fails(self):
        self.report["runner_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "runner"):
            self.check()

    def test_changed_database_fails(self):
        path = self.directory / "capture/plain.accdb"
        path.write_bytes(path.read_bytes()[:-1] + b"x")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.check()

    def test_missing_case_and_partial_error_fail(self):
        cases = self.report["databases"][0]["results"]
        cases[1]["rows"] = []
        with self.assertRaisesRegex(ValueError, "partial rows"):
            self.check()
        cases.pop()
        with self.assertRaisesRegex(ValueError, "cases"):
            self.check()

    def test_lossy_unicode_or_null_empty_substitution_fails(self):
        stored = self.report["databases"][0]["results"][0]
        stored["rows"][1][1] = {"kind": "null"}
        with self.assertRaisesRegex(ValueError, "stored values changed"):
            self.check()
        self.assertEqual(oracle.utf16("😀\ud800"), "d83dde00d800")
        self.assertFalse(oracle.valid_cell({"kind": "text", "value": "d83dG000"}))

    def test_long_strings_not_prefix_truncated(self):
        table = oracle.request()["tables"][1]
        stored = next(c for c in self.report["databases"][0]["results"] if c["id"] == "LongValues.stored")
        self.assertGreater(max(len(x or "") for x in table["values"]), 4 * 1024)
        stored["rows"][-1][1]["value"] = stored["rows"][-1][1]["value"][:4 * 255]
        with self.assertRaisesRegex(ValueError, "stored values changed"):
            self.check()

    def test_non_native_and_missing_error_codes_fail(self):
        case = self.report["databases"][0]["results"][1]
        case["status"] = "skipped"
        with self.assertRaisesRegex(ValueError, "outcome"):
            self.check()
        case["status"] = "error"
        del case["hresult"]
        with self.assertRaisesRegex(ValueError, "HRESULT"):
            self.check()

    def test_incomplete_pairs_sorting_and_groups_fail(self):
        table = {"values": [None, "0061"]}
        integer = lambda n: {"kind": "integer", "value": str(n)}
        for suffix, columns, rows in [
            ("pairs", ["LID", "RID", "Eq", "Lt", "Gt"], []),
            ("sort_asc", ["ID"], [[integer(0)], [integer(0)]]),
            ("group", ["FirstID", "N"], [[integer(0), integer(1)]]),
        ]:
            with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                oracle.check_shape({"id": "ShortValues." + suffix, "columns": columns, "rows": rows}, table)
        oracle.check_shape({"id": "ShortValues.sort_asc", "columns": ["ID"], "rows": [[integer(1)], [integer(0)]]}, table)


if __name__ == "__main__":
    unittest.main()
