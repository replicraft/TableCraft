"""Domain retrieval against original statistical fixtures and independent row models.

Generate inputs with compare_saved_statistics.py --generate first. This script
uses Jackcess only for inert row getters, never for SQL execution.
"""
import argparse
from fractions import Fraction
import hashlib
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
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-domain-oracle-') as classes:
        subprocess.run(['javac', '-encoding', 'UTF-8', '-cp', str(args.jar), '-d', classes,
                        str(RECIPES / 'TableDataOracle.java')], check=True)
        for _, suffix in PROFILES:
            path = args.directory / ('statistics-' + suffix)
            original = path.read_bytes()
            raw = subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes,
                                           'TableDataOracle', str(path), 'AData'], text=True, encoding='utf-8')
            values = [int(row.split('\t')[1].removeprefix('I32:')) for row in raw.splitlines()]
            assert values == [1, 2, 4]
            calls = ['DAvg', 'DCount', 'DFirst', 'DLast', 'DLookup', 'DMin', 'DMax', 'DSum', 'DVar', 'DVarP', 'DStDev', 'DStDevP']
            sql = 'SELECT ' + ','.join(f"{name}('ID','AData') AS C{i}" for i, name in enumerate(calls))
            expected = [Fraction(sum(values), len(values)), len(values), values[0], values[-1], values[0], min(values), max(values), sum(values), *moments(values)]
            cases = [
                (sql, [[f'C{i}' for i in range(len(calls))], expected]),
                ("SELECT DCount('*','AData','ID>1') AS N,DSum('ID','AData','ID>1') AS S", [['N', 'S'], [2, 6]]),
                ("SELECT DCount('*','AData','ID>9') AS N,DSum('ID','AData','ID>9') AS S", [['N', 'S'], [0, None]]),
                ("SELECT DLookup('ID','AData','ID=2') AS V,DCount('*','AData','Missing=1') AS N", [['V', 'N'], [2, None]]),
                ("SELECT DSum('V','StatsDerived') AS V", [['V'], [moments(values)[0]]]),
                ("SELECT DCount('*','StatsCrosstab') AS N", [['N'], [1]]),
                ("SELECT DStDevP('ID','AData','ID=1') AS S,DVarP('ID','AData','ID=1') AS V", [['S', 'V'], [None, None]]),
                ("SELECT DCount('*','StatsParams')", None),
                ("SELECT 1,DSum('1/(ID-2)','AData')", None),
            ]
            for sql, expected in cases:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(path)], capture_output=True, text=True, encoding='utf-8')
                if expected is None:
                    assert result.returncode == 2 and not result.stdout, (sql, result)
                else:
                    assert result.returncode == 0, (sql, result.stderr)
                    verify(result.stdout, expected)
                controls += 1
            assert path.read_bytes() == original
            print(path.name, hashlib.sha256(original).hexdigest(), flush=True)
    print(f'Passed {controls} domain getter/model/direct/named/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
