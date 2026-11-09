"""Original stored domain metadata, independent getters and rational models.

Jackcess only writes/reads inert definitions and renders SQL; it never executes it.
"""
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
from compare_saved_statistics import moments, verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'directory'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-saved-domain-oracle-') as classes:
        names = ['SavedForestFixture', 'SavedStatisticsFixture', 'SavedDomainFixture', 'SavedReadOracle', 'SavedQueryOracle', 'TableDataOracle']
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        *[str(RECIPES / (name + '.java')) for name in names]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                                           name, *map(str, values)], text=True, encoding='utf-8')

        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True, encoding='utf-8')
            assert result.returncode == (0 if accepted else 2), (values, result.stderr)
            if not accepted:
                assert not result.stdout, (values, result.stdout)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedDomainFixture', args.directory / ('domains-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('domains-' + suffix)
            original = path.read_bytes()
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in java('TableDataOracle', path, 'AData').splitlines()]
            assert values == [1, 2, 4]
            unicode = java('TableDataOracle', path, 'Commandes été').splitlines()
            assert [int(row.split('\t')[1].removeprefix('I32:')) for row in unicode] == [3, 7]
            expected = {
                'DomainAll': [[*[f'C{i}' for i in range(12)], 'N'], [Fraction(sum(values), len(values)), len(values), values[0], values[-1], values[0], min(values), max(values), sum(values), *moments(values), len(values)]],
                'DomainCriteria': [['V'], [1]],
                'DomainMissingField': [['V'], [None]],
                'DomainUnicode': [['V'], [10]],
                'DomainNamed': [['V'], [moments(values)[0]]],
                'DomainParams': [['V'], [6]],
                'DomainGrouped': [['G', 'N', 'Rows'], [0, 2, 2], [1, 1, 1]],
                'DomainDerived': [['V'], [7]],
                'DomainUnion': [['V'], [1], [4]],
                'DomainCrosstab': [['G', 'Total', '1', '2', '4'], [0, 6, None, 3, 3], [1, 3, 3, None, None]],
            }
            expected['DomainOmitted'] = expected['DomainAll']
            refusals = {'DomainParamRefuse', 'DomainRecursive', 'DomainUnknown', 'DomainLateError'}
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 38
            names = [name for name in names if name.startswith('Domain')]
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
                    if name == 'DomainParams':
                        command += ['--param', 'Criteria=text:ID>1']
                    result = cli(*command, accepted=name not in refusals)
                    if name not in refusals:
                        verify(result, expected[name])
                    controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} saved-domain getter/rendering/direct/saved/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
