"""Portable integrity/comparison checks; synthetic captures are not native results."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1] / "docs/testing/native_aggregate_oracle.py"
SPEC = importlib.util.spec_from_file_location("native_aggregate_oracle", SOURCE)
oracle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(oracle)


class NativeAggregateEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name) / "probes"
        oracle.prepare(self.directory)
        capture = self.directory / "capture"
        capture.mkdir()
        raw = bytearray(4096)
        raw[4:19] = b"Standard ACE DB"
        (capture / "aggregates.accdb").write_bytes(raw)
        self.report = {
            "format": oracle.FORMAT,
            "request_sha256": hashlib.sha256(oracle.request_bytes()).hexdigest(),
            "runner_sha256": hashlib.sha256(SOURCE.with_name("Capture-NativeAggregates.ps1").read_bytes()).hexdigest(),
            "environment": {"engine_version": "synthetic", "engine_dll_version": "synthetic",
                            "windows_version": "synthetic", "powershell_version": "synthetic", "process_bits": 64},
            "database": {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), "header_hex": raw[:128].hex()},
            "results": [{
                "id": case["id"], "status": "ok",
                "fields": [{"name": "N", "dao_type": 4, "size": 4}, {"name": "Subtype", "dao_type": 3, "size": 2}],
                "rows": [[{"kind": "integer", "value": "1"}, {"kind": "integer", "value": "3"}]],
            } for case in oracle.request()["cases"]],
        }

    def save(self):
        (self.directory / "capture/results.json").write_text(json.dumps(self.report), encoding="utf-8")

    def validate(self):
        self.save()
        return oracle.validate(self.directory)

    def test_recipe_and_runner_are_bound_to_original_source(self):
        self.assertEqual(len(oracle.request()["cases"]), 100)
        self.assertEqual(len({case["id"] for case in oracle.request()["cases"]}), 100)
        digest = hashlib.sha256(oracle.request_bytes()).hexdigest()
        self.assertIn("$recipeSha256 = '" + digest + "'", SOURCE.with_name("Capture-NativeAggregates.ps1").read_text())
        with self.assertRaises(FileExistsError):
            oracle.prepare(self.directory)
        self.assertEqual(self.validate()["captured"], 100)
        self.report["runner_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "runner hash"):
            self.validate()

    def test_changed_request_and_original_database_are_refused(self):
        (self.directory / "request.json").write_bytes(oracle.request_bytes() + b" ")
        with self.assertRaisesRegex(ValueError, "original recipe"):
            self.validate()
        (self.directory / "request.json").write_bytes(oracle.request_bytes())
        path = self.directory / "capture/aggregates.accdb"
        path.write_bytes(path.read_bytes()[:-1] + b"x")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.validate()

    def test_build_case_coverage_and_partial_results_are_checked(self):
        original = copy.deepcopy(self.report)
        mutations = [
            lambda r: r["environment"].pop("engine_dll_version"),
            lambda r: r["results"].pop(),
            lambda r: r["results"][0].update(rows=[]),
            lambda r: r["results"][0]["fields"][0].pop("dao_type"),
            lambda r: r["results"][0].update(status="error", hresult=-1, dao_numbers=[3071]),
            lambda r: r["results"][0].update(status="skipped"),
        ]
        for mutate in mutations:
            self.report = copy.deepcopy(original)
            mutate(self.report)
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                self.validate()

    def test_native_errors_are_observations_not_successes(self):
        self.report["results"][0] = {"id": self.report["results"][0]["id"], "status": "error",
                                     "hresult": -2147467259, "dao_numbers": [3071]}
        counts = self.validate()
        self.assertEqual(counts["captured"], 99)
        self.assertEqual(counts["errors"], 1)
        self.report["results"][0]["dao_numbers"] = []
        with self.assertRaisesRegex(ValueError, "DAO error"):
            self.validate()

    def test_lossless_numeric_encodings_and_signed_zero(self):
        for cell in [
            {"kind": "double", "value": "7ff0000000000000"},
            {"kind": "single", "value": "nan"},
            {"kind": "decimal", "value": "000000010000000000000000001d0000"},
            {"kind": "decimal", "value": "00000001000000000000000000000001"},
            {"kind": "integer", "value": "01"},
        ]:
            self.assertFalse(oracle.valid_cell(cell), cell)
        negative_zero = {"kind": "double", "value": "8000000000000000"}
        self.assertTrue(oracle.valid_cell(negative_zero))
        self.assertTrue(oracle.value_matches(negative_zero, "-0"))
        self.assertFalse(oracle.value_matches(negative_zero, "0"))
        decimal = {"kind": "decimal", "value": "ffffffffffffffffffffffff00000000"}
        self.assertTrue(oracle.value_matches(decimal, "79228162514264337593543950335"))
        self.assertFalse(oracle.value_matches(decimal, "79228162514264337593543950334"))

    def test_comparison_separates_values_runtime_carriers_and_process_failures(self):
        self.save()
        success = SimpleNamespace(returncode=0, stdout="N\tSubtype\n1\t3\n")
        with patch.object(oracle.subprocess, "run", return_value=success):
            result = oracle.compare(self.directory, Path("tablecraft"))
        self.assertEqual(result["counts"]["both_match"], 100)
        different_carrier = SimpleNamespace(returncode=0, stdout="N\tSubtype\n1\t20\n")
        with patch.object(oracle.subprocess, "run", return_value=different_carrier):
            result = oracle.compare(self.directory, Path("tablecraft"))
        self.assertEqual(result["counts"]["value_matches"], 100)
        self.assertEqual(result["counts"]["carrier_matches"], 0)
        for output, expected in [
            (SimpleNamespace(returncode=2, stdout=""), "tablecraft_errors"),
            (SimpleNamespace(returncode=0, stdout="N\tSubtype\n"), "protocol_errors"),
            (SimpleNamespace(returncode=0, stdout='N\tSubtype\n"unterminated'), "protocol_errors"),
        ]:
            with patch.object(oracle.subprocess, "run", return_value=output):
                result = oracle.compare(self.directory, Path("tablecraft"))
            self.assertEqual(result["counts"][expected], 100)
            self.assertNotIn("both_match", result["counts"])

    def test_field_bounds_and_native_carrier_serialization_agree(self):
        self.report["results"][0]["fields"][0]["dao_type"] = 2
        self.report["results"][0]["rows"][0][0]["value"] = "256"
        with self.assertRaisesRegex(ValueError, "carrier mismatch"):
            self.validate()
        self.report["results"][0]["rows"][0][0] = {"kind": "double", "value": "3ff0000000000000"}
        with self.assertRaisesRegex(ValueError, "carrier mismatch"):
            self.validate()

    def test_timeouts_are_failures_not_matches(self):
        self.save()
        with patch.object(oracle.subprocess, "run", side_effect=oracle.subprocess.TimeoutExpired("tablecraft", 10)):
            result = oracle.compare(self.directory, Path("tablecraft"))
        self.assertEqual(result["counts"]["tablecraft_errors"], 100)
        self.assertNotIn("both_match", result["counts"])

    def test_comparison_checks_that_tablecraft_did_not_modify_the_original(self):
        self.save()
        path = self.directory / "capture/aggregates.accdb"

        def mutated(*args, **kwargs):
            path.write_bytes(path.read_bytes() + b"x")
            return SimpleNamespace(returncode=2, stdout="")

        with patch.object(oracle.subprocess, "run", side_effect=mutated):
            with self.assertRaisesRegex(ValueError, "changed original"):
                oracle.compare(self.directory, Path("tablecraft"))


if __name__ == "__main__":
    unittest.main()
