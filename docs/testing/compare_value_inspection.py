"""Independent subtype/bounds goldens and inert native type-inspection metadata."""
import argparse
import csv
import hashlib
import json
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES


def verify(output, expected):
    actual = list(csv.reader(StringIO(output), delimiter='\t', strict=True))
    # Type names are ASCII Text display values inside the TSV transport.
    # Decode their quoted display spelling separately from NULL/Boolean/numbers.
    for row in actual[1:]:
        for index, cell in enumerate(row):
            if cell.startswith('"'):
                row[index] = json.loads(cell)
    assert actual == [[str(cell) for cell in row] for row in expected], (actual, expected)


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
            assert not result.stdout
        return result.stdout

    # These are explicit expected public constants, independent of production enums.
    for operand, name, code in [
        ('Null', 'Null', 1), ('CInt(1)', 'Integer', 2), ('CLng(1)', 'Long', 3),
        ('CSng(1)', 'Single', 4), ('CDbl(1)', 'Double', 5), ('CCur(1)', 'Currency', 6),
        ('CDate(0)', 'Date', 7), ("'secret'", 'String', 8), ('True', 'Boolean', 11),
        ('CDec(1)', 'Decimal', 14), ('CByte(1)', 'Byte', 17), ('CLngLng(1)', 'LongLong', 20),
    ]:
        sql = f'SELECT TypeName({operand}) AS T,VarType({operand}) AS V,IsArray({operand}) AS A,IsEmpty({operand}) AS E,IsError({operand}) AS F'
        for command in ['eval', 'run']:
            verify(cli('query', command, sql), [['T', 'V', 'A', 'E', 'F'], [name, code, 'FALSE', 'FALSE', 'FALSE']])
            controls += 1

    for count in [0, 1, 2, 10, 50]:
        array = 'Array(' + ','.join(str(value) for value in range(count)) + ')'
        sql = f'SELECT TypeName({array}) AS T,VarType({array}) AS V,IsArray({array}) AS A,LBound({array}) AS L,UBound({array}) AS U'
        for command in ['eval', 'run']:
            verify(cli('query', command, sql), [['T', 'V', 'A', 'L', 'U'], ['Variant()', 8204, 'TRUE', 0, count - 1]])
            controls += 1

    for command in ['eval', 'run']:
        for sql in [
            'SELECT IsError(1/0)', 'SELECT IsArray(Array(1,1/0))',
            'SELECT UBound(Array(1),2)', 'SELECT UBound(Null,1/0)',
            'SELECT TypeName(Array(Array(1)))',
        ]:
            cli('query', command, sql, accepted=False)
            controls += 1

    with tempfile.TemporaryDirectory(prefix='tablecraft-inspection-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedInspectionFixture',
                 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output([
                'java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                name, *map(str, values),
            ], text=True, encoding='utf-8')

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedInspectionFixture', args.directory / ('inspection-' + suffix), profile)

        for _, suffix in PROFILES:
            path = args.directory / ('inspection-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:'))
                      for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            expected = {
                'MetaScalars': [['T', 'V', 'A', 'E', 'F'], ['Integer', 2, 'FALSE', 'FALSE', 'FALSE']],
                'MetaArray': [['T', 'V', 'A', 'L', 'U'], ['Variant()', 8204, 'TRUE', 0, 2]],
                'MetaEmptyArray': [['T', 'V', 'A', 'L', 'U'], ['Variant()', 8204, 'TRUE', 0, -1]],
                'MetaParams': [['T', 'V'], ['Long', 3]],
                'MetaRows': [['ID', 'T', 'V', 'L', 'U'], *[[v, 'Long', 3, 0, 1] for v in values]],
                'MetaGrouped': [['G', 'V'], [0, 1], [1, 1]],
                'MetaCrosstab': [['G', 'Total', '1', '2', '4'], [0, 2, 'NULL', 1, 1], [1, 1, 1, 'NULL', 'NULL']],
                'MetaDerived': [['V'], [8204]],
                'MetaUnion': [['V'], ['Integer'], ['Double']],
            }
            refusals = {'MetaBadDimension', 'MetaError', 'MetaLate'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 26
            names = [name for name in names if name.startswith('Meta')]
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
                    if name == 'MetaParams':
                        command += ['--param', 'P=long:21']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1

            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)

    print(f'Passed {controls} subtype/bounds/scalar/getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
