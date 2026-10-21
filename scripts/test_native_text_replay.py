"""Synthetic replay regressions, never evidence of native Access compatibility."""
import csv
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import unittest

RECIPES = Path(__file__).resolve().parents[1] / "docs/testing"
sys.path.insert(0, str(RECIPES))
import replay_native_text as replay

SPEC = importlib.util.spec_from_file_location("text_evidence_tests", Path(__file__).with_name("test_native_text_oracle.py"))
evidence_tests = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(evidence_tests)


def integer(value):
    return {"kind": "integer", "value": str(value)}


def text(value):
    return {"kind": "text", "value": replay.oracle.utf16(value)}


def encoded(columns, rows):
    output = io.StringIO(newline="")
    writer = csv.writer(output, delimiter="\t", lineterminator="\n")
    writer.writerow(columns)
    writer.writerows(rows)
    return output.getvalue().encode("utf-8")


class ReplayValueTests(unittest.TestCase):
    def test_lossless_display_and_transport(self):
        values = [{"kind": "null"}, {"kind": "boolean", "value": True}, integer(-1),
                  text("NULL"), text(""), text('a\x00b\t\n\\"😀\x85')]
        shown = [replay.cell_display(value) for value in values]
        self.assertEqual(shown, ["NULL", "TRUE", "-1", '"NULL"', '""', '"a\\u{0000}b\\t\\n\\\\\\"😀\\u{0085}"'])
        self.assertEqual(replay.decode_output(encoded(["V"], [[cell] for cell in shown])),
                         (["V"], [[cell] for cell in shown]))
        with self.assertRaises(UnicodeDecodeError):
            replay.cell_display({"kind": "text", "value": "d800"})

    def test_malformed_transport_is_not_a_match(self):
        for raw in [b"", b'V\n"unterminated', b"V\n1\t2\n", b"V\n\xff\n",
                    b"x" * (replay.MAX_OUTPUT_BYTES + 1)]:
            with self.subTest(raw=raw[:20]), self.assertRaises((ValueError, csv.Error, UnicodeError)):
                replay.decode_output(raw)

    def test_order_types_null_and_long_suffix_remain_significant(self):
        case = {"id": "LongValues.stored", "columns": ["ID", "V"],
                "rows": [[integer(0), text("x" * 1024 + "a")], [integer(1), {"kind": "null"}]]}
        rows = [[replay.cell_display(cell) for cell in row] for row in case["rows"]]
        self.assertEqual(replay.compare_rows(case, ["ID", "V"], rows, {}, None), "match")
        for changed in [rows[::-1], [["0", '"' + "x" * 1024 + 'b"'], rows[1]],
                        [rows[0], ["1", '"NULL"']]]:
            self.assertEqual(replay.compare_rows(case, ["ID", "V"], changed, {}, None), "rows_mismatch")
        self.assertEqual(replay.compare_rows(case, ["ID", "Wrong"], rows, {}, None), "columns_mismatch")

    def test_distinct_uses_observed_classes_not_representative_spelling(self):
        table = {"values": [None, replay.oracle.utf16("a"), replay.oracle.utf16("A"), replay.oracle.utf16("b")]}
        pairs = {"status": "ok", "rows": []}
        groups = [0, 1, 1, 2]
        for left in range(4):
            for right in range(4):
                eq = {"kind": "null"} if left == 0 or right == 0 else {"kind": "boolean", "value": groups[left] == groups[right]}
                pairs["rows"].append([integer(left), integer(right), eq])
        case = {"id": "ShortValues.distinct", "columns": ["V"],
                "rows": [[{"kind": "null"}], [text("a")], [text("b")]]}
        self.assertEqual(replay.compare_rows(case, ["V"], [['"b"'], ['"A"'], ["NULL"]], table, pairs),
                         "match_equivalent_representatives")
        self.assertEqual(replay.compare_rows(case, ["V"], [['"A"'], ['"A"'], ["NULL"]], table, pairs), "rows_mismatch")
        self.assertEqual(replay.compare_rows(case, ["V"], [['"a"'], ['"c"'], ["NULL"]], table, pairs), "rows_mismatch")
        self.assertEqual(replay.compare_rows(case, ["V"], [['"A"'], ['"b"'], ["NULL"]], table, None),
                         "distinct_representatives_unresolved")
        # Native DISTINCT kept both case variants: equality is insufficient to
        # explain this operation. Never merge them using a guessed policy.
        case["rows"].append([text("A")])
        self.assertEqual(replay.compare_rows(case, ["V"], [['"a"'], ['"b"'], ["NULL"], ['"a"']], table, pairs),
                         "distinct_representatives_unresolved")
        pairs["rows"][6][2] = {"kind": "boolean", "value": False}
        self.assertIsNone(replay.distinct_classes(table, pairs))


class ReplayProcessTests(unittest.TestCase):
    def setUp(self):
        self.fixture = evidence_tests.NativeTextEvidenceTests("test_errors_remain_errors_not_passes")
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.check()
        self.directory = self.fixture.directory
        self.cli = self.directory / "synthetic-cli"
        self.cli.write_bytes(b"not a real executable")
        self.calls = []

    def simulate(self, command, **options):
        self.calls.append(command)
        self.assertEqual(options, {"capture_output": True, "timeout": replay.TIMEOUT_SECONDS})
        sql = command[3]
        if "ORDER BY ID" in sql and sql.startswith("SELECT ID, V FROM"):
            name = "ShortValues" if "ShortValues" in sql else "LongValues"
            case = next(c for c in self.fixture.report["databases"][0]["results"] if c["id"] == name + ".stored")
            rows = [[replay.cell_display(cell) for cell in row] for row in case["rows"]]
            return subprocess.CompletedProcess(command, 0, encoded(case["columns"], rows), b"")
        return subprocess.CompletedProcess(command, 2, b"", b"synthetic failure")

    def test_replays_all_cases_and_errors_never_count_as_matches(self):
        report = replay.replay(self.directory, self.cli, "ordinal", "ansi89", self.simulate)
        self.assertEqual(len(self.calls), 76)
        self.assertEqual(report["counts"], {"match": 4, "native_error_unresolved": 72})
        self.assertFalse(report["all_matched"])
        self.assertEqual(sum("--param" in command for command in self.calls), 4)
        self.assertTrue(all(command[-2:] == ["--param", "P=text:a "] for command in self.calls if "--param" in command))
        self.assertTrue(all("--collation" in command and "--like-mode" in command for command in self.calls))

    def test_timeout_partial_stdout_and_candidate_error_are_retained(self):
        def timeout(command, **options):
            raise subprocess.TimeoutExpired(command, options["timeout"])
        report = replay.replay(self.directory, self.cli, "ordinal", "ansi89", timeout)
        self.assertEqual(report["counts"], {"timeout": 76})
        for stdout, status in [(b"partial", "partial_output_on_error"), (b"", "candidate_error")]:
            with self.subTest(stdout=stdout):
                def failed(command, **options):
                    return subprocess.CompletedProcess(command, 2, stdout, b"failed")
                report = replay.replay(self.directory, self.cli, "ordinal", "ansi89", failed)
                self.assertEqual(report["counts"][status], 76 if stdout else 4)

    def test_input_validation_precedes_execution_and_mutation_is_fatal(self):
        request = self.directory / "request.json"
        original = request.read_bytes()
        request.write_bytes(b"invalid")
        with self.assertRaisesRegex(ValueError, "recipe"):
            replay.replay(self.directory, self.cli, "ordinal", "ansi89", self.simulate)
        self.assertEqual(self.calls, [])
        request.write_bytes(original)
        def mutate(command, **options):
            request.write_bytes(original + b" ")
            return self.simulate(command, **options)
        with self.assertRaisesRegex(ValueError, "changed protected input"):
            replay.replay(self.directory, self.cli, "ordinal", "ansi89", mutate)


if __name__ == "__main__":
    unittest.main()
