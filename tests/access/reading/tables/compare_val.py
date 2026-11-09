"""Independent bounded Val controls; Python integers/Decimal are the models.

No database is modified or active Access content executed. Native Access is unrun.
"""
import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    controls = [("2457", 2457.0), ("    1615 198th Street N.E.", 1615198.0),
                ("&HFFFF", -1.0), ("50%", 50.0), ("24 and 57", 24.0)]
    for _ in range(512):
        signed = rng.randrange(-(2**31), 2**31)
        unsigned = rng.randrange(2**32)
        controls.append((f'{signed}&tail', float(signed)))
        controls.append((f'&H{unsigned:X}&', float(unsigned if unsigned < 2**31 else unsigned-2**32)))
        coefficient = rng.randrange(-10**12, 10**12)
        spelling = str(Decimal(coefficient).scaleb(-6))
        expected = Decimal(spelling).quantize(Decimal('0.0001'), rounding=ROUND_HALF_EVEN)
        controls.append((spelling+'@', float(expected)))
    count = 0
    for mode in ['eval', 'run']:
        for start in range(0, len(controls), 16):
            batch = controls[start:start+16]
            sql = 'SELECT '+','.join("Val('"+text.replace("'", "''")+f"') AS V{i}" for i,(text,_) in enumerate(batch))
            result = subprocess.run([str(args.cli), 'query', mode, sql], capture_output=True, check=True, text=True)
            rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
            assert len(rows) == 2 and len(rows[1]) == len(batch)
            for cell,(text,expected) in zip(rows[1], batch):
                assert float(cell) == expected, (mode, text, cell, expected)
                count += 1
        for text in ['50.5%', '32768%', '2147483648&', '9223372036854775808^', '1e400', '1e40!', '1$']:
            result = subprocess.run([str(args.cli), 'query', mode, "SELECT 1,Val('"+text+"')"], capture_output=True)
            assert result.returncode != 0 and not result.stdout, text
            count += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                     RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        expected = []
        for line in tagged.splitlines():
            row = line.split('\t')[1:]
            identity = int(row[0].removeprefix('I32:'))
            text = bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
            expected.append((identity, float(int(text))))
        expected.sort()
        sql = 'SELECT ORDR_NBR AS N,Val(NBR_VALUE) AS V FROM ROULETTE_TB ORDER BY N'
        for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
            output = run(args.cli, 'query', 'run', sql, '--database', args.database, *options)
            actual = [(int(row[0]), float(row[1])) for row in csv.reader(StringIO(output), delimiter='\t') if row[0] != 'N']
            assert actual == expected
            count += len(expected)
        assert digest(args.database) == before
    print(f'Passed {count} independent integer/radix/Currency/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
