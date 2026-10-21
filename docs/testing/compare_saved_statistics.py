"""Original stored metadata plus exact-rational statistical models; not Access execution."""
import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import math
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES


def moments(values):
    n = len(values)
    if not n:
        return [None] * 4
    mean = Fraction(sum(values), n)
    squares = sum((Fraction(value) - mean) ** 2 for value in values)
    sample = squares / (n - 1) if n > 1 else None
    population = squares / n
    # Follow the distinct singleton rules in Microsoft's Var/VarP and StDevP
    # references; Python's population variance definition alone would return 0.
    return [sample, population if n > 1 else None,
            math.sqrt(sample) if sample is not None else None, math.sqrt(population)]


def verify(output, expected):
    rows = list(csv.reader(StringIO(output), delimiter='\t', strict=True))
    assert rows[0] == expected[0], (rows, expected)
    assert len(rows) == len(expected), (rows, expected)
    for actual, wanted in zip(rows[1:], expected[1:]):
        assert len(actual) == len(wanted)
        for cell, value in zip(actual, wanted):
            if value is None:
                assert cell == 'NULL', (cell, value)
            else:
                assert math.isclose(float(cell), float(value), rel_tol=2e-13, abs_tol=1e-300), (cell, value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'directory'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-statistics-oracle-') as classes:
        sources = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes, *[str(RECIPES / (name + '.java')) for name in sources]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes, name, *map(str, values)], text=True, encoding='utf-8')

        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True, encoding='utf-8')
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and not result.stdout, (values, result)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedStatisticsFixture', args.directory / ('statistics-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('statistics-' + suffix)
            original = path.read_bytes()
            records = java('TableDataOracle', path, 'AData').splitlines()
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in records]
            assert values == [1, 2, 4]
            headers = ['V', 'VP', 'S', 'SP']
            all_values = [headers, moments(values)]
            expected = {
                'StatsAll': all_values, 'StatsDerived': all_values, 'StatsParams': all_values,
                'StatsGrouped': [headers + ['G']] + [moments([v for v in values if v % 2 == key]) + [key] for key in [0, 1]],
                'StatsSingleton': [headers, moments([1])], 'StatsEmpty': [headers, moments([])],
                'StatsCrosstab': [['Bucket', 'Total', '0', '1'], [0, moments(values)[0], 1, 0]],
                'StatsUnion': [['V'], [moments(values)[1]], [moments(values)[3]]],
            }
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 23
            names = [name for name in names if name.startswith('Stats')]
            assert set(names) == set(expected) | {'StatsWrongType'}
            for name in names:
                raw = java('SavedQueryOracle', path, name)
                inspected = cli('inspect', path, '--query', name)
                assert list(csv.reader(StringIO(inspected), delimiter='\t', strict=True)) == [line.split('\t') for line in raw.splitlines()]
                sql = java('SavedReadOracle', path, name)
                assert cli('query', 'saved', '--database', path, name) == cli('query', 'parse', sql)
                controls += 2
                for query in [sql, None, 'SELECT * FROM [' + name + ']']:
                    command = ['query', 'run'] + (['--saved', name] if query is None else [query]) + ['--database', path]
                    if name == 'StatsParams':
                        command += ['--param', 'Shift=long:1000']
                    accepted = name != 'StatsWrongType'
                    result = cli(*command, accepted=accepted)
                    if accepted:
                        verify(result, expected[name])
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} statistical getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
