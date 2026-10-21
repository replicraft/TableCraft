"""Exercise actual large Binary transport and malformed quoted oracle output."""
import csv
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "docs/testing/cli_tsv.py"
SPEC = importlib.util.spec_from_file_location("cli_tsv", SOURCE)
transport = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(transport)


class TsvOracleTests(unittest.TestCase):
    def test_maximum_binary_cell_and_restored_parser_limit(self):
        previous = csv.field_size_limit(131072)
        self.addCleanup(csv.field_size_limit, previous)
        binary = "0x" + "2a" * 65536
        raw = 'Name\tData\n"a""b"\t' + binary + '\n'
        self.assertEqual(transport.display_tsv(raw), 'Name\tData\na"b\t' + binary + '\n')
        self.assertEqual(csv.field_size_limit(), 131072)

    def test_malformed_quoting_fails_and_restores_limit(self):
        previous = csv.field_size_limit()
        with self.assertRaises(csv.Error):
            transport.display_tsv('"unterminated')
        self.assertEqual(csv.field_size_limit(), previous)

    def test_display_escapes_and_empty_values_remain_literal(self):
        self.assertEqual(transport.display_tsv('Name\tData\r\n"a\\tb"\t0x\r\n""\tNULL\r\n'), 'Name\tData\na\\tb\t0x\n\tNULL\n')


if __name__ == "__main__":
    unittest.main()
