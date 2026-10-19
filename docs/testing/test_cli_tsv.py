"""Transport checks for independent fixture comparison adapters."""
import unittest
from cli_tsv import command_output, display_tsv


class CliTsvTests(unittest.TestCase):
    def test_transport_quotes_preserve_escaped_display_and_section_widths(self):
        encoded = 'Name\tValue\nLabel\t"""雪\\n\\t"""\nA\tB\tC\n'
        self.assertEqual(display_tsv(encoded), 'Name\tValue\nLabel\t"雪\\n\\t"\nA\tB\tC\n')

    def test_tagged_oracles_sql_and_bordered_output_bypass_tsv_decoding(self):
        text = '"quoted" text\n'
        for args in [
            ('java', '-cp', 'jar', 'TableDataOracle'),
            ('native_rows', 'example.accdb', 'Items'),
            ('tablecraft', 'query', 'saved', 'Name'),
            ('tablecraft', 'rows', 'example.accdb', 'Items', '--format', 'table'),
        ]:
            self.assertEqual(command_output(args, text), text)


if __name__ == '__main__':
    unittest.main()
