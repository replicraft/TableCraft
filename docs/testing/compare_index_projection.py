"""Compare mixed-schema indexed scalar reads and exact integer range queries."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import display


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'ace2007', 'ace2016', 'ace2019'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-index-projection-') as directory:
        subprocess.run(['javac', '-cp', str(args.jar), '-d', directory, 'docs/testing/MultivalueOracle.java', 'docs/testing/ScalarProjectionOracle.java'], check=True)
        classpath = str(args.jar) + os.pathsep + directory
        for family in ('2007', '2016', '2019'):
            path = getattr(args, 'ace' + family)
            before = path.read_bytes()
            fields = ['ID', 'Notes', 'Score', 'Label'] + ([] if family == '2007' else ['Big'])
            tagged = subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', classpath, 'ScalarProjectionOracle', str(path), 'MixedIndexed', *fields], text=True, encoding='utf-8')
            rows = [list(map(display, line.split('\t'))) for line in tagged.splitlines()]
            assert len(rows) == 1000 and {row[0] for row in rows} >= {'NULL', str(-(2**31)), str(2**31-1)}
            indexes = ['IdAsc', 'IdDesc', 'IdScore', 'ScoreKey'] + ([] if family == '2007' else ['BigAsc', 'BigDesc'])
            def query(sql, index):
                result = subprocess.run(list(map(str, [args.cli, 'query', 'run', sql, '--database', path, '--index', 'MixedIndexed', index, '--format', 'tsv'])), capture_output=True, text=True, encoding='utf-8')
                assert result.returncode == 0 and not result.stderr, result.stderr
                return list(csv.reader(io.StringIO(result.stdout), delimiter='\t', strict=True))
            for index in indexes:
                ordered = subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', classpath, 'ScalarProjectionOracle', str(path), 'MixedIndexed', '--index', index, *fields], text=True, encoding='utf-8')
                expected = [list(map(display, line.split('\t'))) for line in ordered.splitlines()]
                assert query('SELECT ' + ', '.join(fields) + ' FROM MixedIndexed', index) == [fields] + expected, (family, index)
                assert query('SELECT Count(*) AS N FROM MixedIndexed', index) == [['N'], ['1000']]
                controls += 2
            cases = [
                ('ID IS NULL', lambda row: row[0] == 'NULL'),
                ('ID=-40', lambda row: row[0] == '-40'),
                ('ID>=-40 AND ID<20', lambda row: row[0] != 'NULL' and -40 <= int(row[0]) < 20),
                ('ID BETWEEN 20 AND -40', lambda row: row[0] != 'NULL' and -40 <= int(row[0]) <= 20),
                ('ID>2147483646', lambda row: row[0] != 'NULL' and int(row[0]) > 2147483646),
                ('ID<-2147483647', lambda row: row[0] != 'NULL' and int(row[0]) < -2147483647),
                ('ID=777777', lambda row: False),
                ('Score>=400 AND Score<405', lambda row: 400 <= int(row[2]) < 405),
            ]
            for index in ['IdAsc', 'IdDesc', 'IdScore', 'ScoreKey']:
                for predicate, matches in cases:
                    expected = sorted(filter(matches, rows), key=lambda row: int(row[2]))
                    assert query('SELECT ' + ', '.join(fields) + ' FROM MixedIndexed WHERE ' + predicate + ' ORDER BY Score', index) == [fields] + expected, (family, index, predicate)
                    controls += 1
            if family != '2007':
                assert {row[4] for row in rows} >= {str(-(2**63)), str(2**63-1), '9007199254740993'}
                for index in ['BigAsc', 'BigDesc']:
                    for predicate, matches in [
                        ('Big IS NULL', lambda row: row[4] == 'NULL'),
                        ('Big=9007199254740993', lambda row: row[4] == '9007199254740993'),
                        ('Big>9007199254740992 AND Big<9007199254740995', lambda row: row[4] != 'NULL' and 9007199254740992 < int(row[4]) < 9007199254740995),
                        ('Big>9223372036854775806', lambda row: row[4] != 'NULL' and int(row[4]) > 9223372036854775806),
                        ('Big<0', lambda row: row[4] != 'NULL' and int(row[4]) < 0),
                    ]:
                        expected = sorted(filter(matches, rows), key=lambda row: int(row[2]))
                        assert query('SELECT ' + ', '.join(fields) + ' FROM MixedIndexed WHERE ' + predicate + ' ORDER BY Score', index) == [fields] + expected
                        controls += 1
            for index in ['missing', 'SkipNull', 'LabelKey']:
                result = subprocess.run(list(map(str, [args.cli, 'query', 'run', 'SELECT ID FROM MixedIndexed WHERE ID=1', '--database', path, '--index', 'MixedIndexed', index])), capture_output=True)
                assert result.returncode == 2 and not result.stdout
                controls += 1
            assert path.read_bytes() == before
            print(family, len(rows), 'rows; size', len(before), 'unchanged', hashlib.sha256(before).hexdigest(), flush=True)
    print(controls, 'indexed projection/order/range/refusal controls passed; native Access unrun')


if __name__ == '__main__':
    main()
