"""Independent stable-Unicode/ASCII casing controls; native Access remains unrun."""
import argparse
import csv
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_cstr import spelling
from compare_large_tables import digest
from compare_string_search import quoted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    # Older established characters avoid assuming identical Unicode database versions.
    alphabet = 'AbCz09 éÉßﬃΣσςΑΩIİiıÅå中🦀'
    texts = ['', 'Hello World 1234', 'ΟΣ ΟΣΑ', 'I\u0301', 'İ', 'Straße', 'a\tb\nc']
    texts += [''.join(rng.choices(alphabet, k=rng.randrange(1, 9))) for _ in range(128)]
    controls = 0
    for mode in ['eval', 'run']:
        for text in texts:
            sql = f'SELECT LCase({quoted(text)}),UCase({quoted(text)})'
            result = subprocess.run([str(args.cli), 'query', mode, sql, '--case-locale', 'und'], capture_output=True, text=True)
            assert result.returncode == 0, (text, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
            assert rows[1] == [spelling(text.lower()), spelling(text.upper())], (text, rows)
            controls += 2
        for locale, text, lower, upper in [('tr', 'Iİiı', 'ıiiı', 'IİİI'), ('az', 'Iİiı', 'ıiiı', 'IİİI'), ('lt', 'I\u0301', 'i\u0307\u0301', 'I\u0301')]:
            sql = f'SELECT LCase({quoted(text)}),UCase({quoted(text)})'
            result = subprocess.run([str(args.cli), 'query', mode, sql, '--case-locale', locale], capture_output=True, text=True)
            assert result.returncode == 0, result.stderr
            assert list(csv.reader(StringIO(result.stdout), delimiter='\t'))[1] == [spelling(lower), spelling(upper)]
            controls += 2
        for expression in ["LCase('x')", "UCase('')", 'LCase(1)', 'UCase(NULL,1/0)', "VBA.LCase('x')"]:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,' + expression], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            controls += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES / 'TableDataOracle.java', args.database, 'ROULETTE_TB')
        source_rows = sorted((int(parts[1].removeprefix('I32:')), bytes.fromhex(parts[2].removeprefix('T')).decode('utf-8')) for parts in (line.split('\t') for line in tagged.splitlines()))
        expected = [[str(identity), spelling('abc '+text), spelling('STRASSE '+text)] for identity, text in source_rows]
        sql = "SELECT ORDR_NBR AS N,LCase('AbC ' & NBR_VALUE),UCase('straße ' & NBR_VALUE) FROM ROULETTE_TB ORDER BY N"
        for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
            result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--case-locale', 'und', *options], capture_output=True, text=True)
            assert result.returncode == 0, result.stderr
            assert list(csv.reader(StringIO(result.stdout), delimiter='\t'))[1:] == expected
            controls += len(expected)*2
        assert digest(args.database) == before
    print(f'Passed {controls} independent casing/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
