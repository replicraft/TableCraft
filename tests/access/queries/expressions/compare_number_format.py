"""Independent Decimal models for complete numeric Text conversion profiles.

Optional native rows supply input through Jackcess; native Access remains unrun.
"""
import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN, localcontext
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest


def regional(text, profile):
    if profile == 'invariant':
        return text
    value = Decimal(text)
    whole, _, fraction = format(abs(value), 'f').partition('.')
    groups = []
    while whole:
        groups.append(whole[-3:])
        whole = whole[:-3]
    group, point, currency = (',', '.', '$') if profile == 'en-us' else ('.', ',', '€')
    spelling = group.join(reversed(groups))+(point+fraction if fraction else '')
    return ('-' if value < 0 else '')+currency+spelling


def decimal_model(text):
    value = Decimal(text)
    whole = value.copy_abs().adjusted()+1 if value else 0
    scale = min(max(0, -value.as_tuple().exponent), max(0, min(28, 29-whole)))
    while True:
        rounded = value.quantize(Decimal(1).scaleb(-scale), rounding=ROUND_HALF_EVEN)
        if abs(int(rounded.scaleb(scale))) <= 2**96-1:
            return format(rounded, 'f')
        assert scale > 0
        scale -= 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    controls = []
    for _ in range(256):
        integer = rng.randrange(-(2**63), 2**63)
        fixed = Decimal(rng.randrange(-10**16, 10**16)).scaleb(-8)
        text = format(fixed, 'f')
        controls.extend([('CLngLng', str(integer), str(integer)),
                         ('CCur', text, format(fixed.quantize(Decimal('.0001'), rounding=ROUND_HALF_EVEN), 'f')),
                         ('CDec', text, decimal_model(text))])
    for _ in range(256):
        exact = Decimal(rng.randrange(-(2**96)+1, 2**96)).scaleb(-rng.randrange(29))
        text = format(exact, 'f')
        controls.append(('CDec', text, decimal_model(text)))
    for text in ['1.49999999999999999999999999999', '1.50000000000000000000000000001', '-2.5', '3.5']:
        controls.append(('CLng', text, str(int(Decimal(text).to_integral_value(rounding=ROUND_HALF_EVEN)))))
    for text in ['79228162514264337593543950335', '7922816251426433759354395033.6',
                 '1.00000000000000000000000000005', '1.00000000000000000000000000015', '1e-29']:
        controls.append(('CDec', text, decimal_model(text)))
    controls.extend([('CBool', 'true', 'TRUE'), ('CBool', '#FALSE#', 'FALSE')])
    count = 0
    for mode in ['eval', 'run']:
        for profile in ['invariant', 'en-us', 'de-de']:
            cases = [(function, regional(text, profile) if function != 'CBool' else text, expected)
                     for function, text, expected in controls]
            for start in range(0, len(cases), 8):
                batch = cases[start:start+8]
                sql = 'SELECT '+','.join(f"{fn}('{text}') AS C{i}" for i,(fn,text,_) in enumerate(batch))
                result = subprocess.run([str(args.cli), 'query', mode, sql, '--number-format', profile], capture_output=True, text=True)
                assert result.returncode == 0, (mode, profile, sql, result.stderr)
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert len(rows) == 2 and len(rows[1]) == len(batch)
                for cell,(fn,text,expected) in zip(rows[1], batch):
                    assert cell == expected, (profile, mode, fn, text, cell, expected)
                    count += 1
            for expression in ["CLng('12tail')", "CInt('1%')", "CDbl('NaN')", "CCur('9223372036854775808')"]:
                result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression, '--number-format', profile], capture_output=True)
                assert result.returncode != 0 and not result.stdout, (profile, expression)
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
            value = int(bytes.fromhex(row[1].removeprefix('T')).decode('utf-8'))
            expected.append([str(identity), str(value)])
        expected.sort(key=lambda row: int(row[0]))
        sql = 'SELECT ORDR_NBR AS N,CLng(NBR_VALUE) AS V FROM ROULETTE_TB ORDER BY N'
        for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
            for profile in ['invariant', 'en-us', 'de-de']:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--number-format', profile, *options], capture_output=True, check=True, text=True)
                assert list(csv.reader(StringIO(result.stdout), delimiter='\t'))[1:] == expected
                count += len(expected)
        assert digest(args.database) == before
    print(f'Passed {count} independent numeric-conversion/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    with localcontext() as context:
        context.prec = 200
        main()
