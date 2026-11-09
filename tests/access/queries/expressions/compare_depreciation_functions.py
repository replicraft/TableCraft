"""Independent rational depreciation ledgers and original inert native definitions."""
import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES
from compare_saved_statistics import verify


def schedule(cost, salvage, life, factor):
    cost, salvage, factor = map(Fraction, (cost, salvage, factor))
    book = cost
    digits = sum(range(1, life + 1))
    result = []
    for period in range(1, life + 1):
        charge = max(Fraction(0), min(book * factor / life, book - salvage))
        result.append([(cost - salvage) / life, (cost - salvage) * (life - period + 1) / digits, charge])
        book -= charge
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'directory'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0

    def cli(*values, accepted=True):
        result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True, encoding='utf-8')
        assert result.returncode == (0 if accepted else 2), (values, result.stderr)
        if not accepted:
            assert not result.stdout, (values, result.stdout)
        return result.stdout

    headings = ['Straight', 'Weighted', 'Declining']
    for life in [3, 4, 10, 40]:
        for factor in ['1', '1.5', '2']:
            rows = schedule(1200, 200, life, factor)
            for period in sorted({1, 2, life}):
                sql = f"SELECT SLN(1200,200,{life}) AS Straight,SYD(1200,200,{life},{period}) AS Weighted,DDB(1200,200,{life},{period},{factor}) AS Declining"
                for command in ['eval', 'run']:
                    verify(cli('query', command, sql), [headings, rows[period-1]])
                    controls += 1
    for command in ['eval', 'run']:
        for sql in ["SELECT 1,SLN(1200,200,0)", "SELECT 1,SYD(1200,200,4,5)", "SELECT 1,DDB(Null,200,4,1,1/0)"]:
            cli('query', command, sql, accepted=False)
            controls += 1
    with tempfile.TemporaryDirectory(prefix='tablecraft-depreciation-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedDepreciationFixture', 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                                           name, *map(str, values)], text=True, encoding='utf-8')

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedDepreciationFixture', args.directory / ('depreciation-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('depreciation-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            rows = schedule(1200, 200, 4, 2)
            custom = schedule(1200, 200, 4, '1.5')
            expected = {
                'DepreciationAll': [headings, rows[0]],
                'DepreciationRows': [['Period', *headings], *[[v, *rows[v-1]] for v in values]],
                'DepreciationFactor': [['V'], *[[custom[v-1][2]] for v in values]],
                'DepreciationParams': [['V'], [rows[1][2]]],
                'DepreciationGrouped': [['G', 'V'], *[[g, sum(rows[v-1][1] for v in values if v % 2 == g)] for g in [0, 1]]],
                'DepreciationCrosstab': [['G', 'Total', '1', '2', '4'], [0, rows[1][2]+rows[3][2], None, rows[1][2], rows[3][2]], [1, rows[0][2], rows[0][2], None, None]],
                'DepreciationDerived': [['V'], *[[Fraction(1000, v)] for v in values]],
                'DepreciationUnion': [['V'], [250], [600]],
            }
            refusals = {'DepreciationInvalid', 'DepreciationNull'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 24
            names = [name for name in names if name.startswith('Depreciation')]
            assert set(names) == set(expected) | refusals
            for name in names:
                raw = java('SavedQueryOracle', path, name)
                inspected = cli('inspect', path, '--query', name)
                assert list(csv.reader(StringIO(inspected), delimiter='\t', strict=True)) == [line.split('\t') for line in raw.splitlines()]
                sql = java('SavedReadOracle', path, name)
                assert cli('query', 'saved', '--database', path, name) == cli('query', 'parse', sql)
                controls += 2
                for query in [sql, None, 'SELECT * FROM [' + name + ']']:
                    command = ['query', 'run'] + (['--saved', name] if query is None else [query]) + ['--database', path]
                    if name == 'DepreciationParams':
                        command += ['--param', 'Per=long:2']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} depreciation ledger/scalar/getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
