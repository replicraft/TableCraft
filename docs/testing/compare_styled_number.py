"""Check explicit fixed-place numeric profiles with independent Python Decimal models.

Optional Jackcess row inputs do not qualify native Access formatting semantics.
"""
import argparse
import csv
from decimal import Decimal, localcontext, ROUND_HALF_UP
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_cstr import spelling
from compare_large_tables import digest
from compare_string_search import quoted


def expected(value, kind, profile, places=2, leading=True, parentheses=None, grouping=None):
    if parentheses is None:
        parentheses = kind == 'FormatCurrency' and profile == 'en-us'
    if grouping is None:
        grouping = profile != 'invariant'
    with localcontext() as context:
        context.prec = 1024
        if kind == 'FormatPercent':
            value *= 100
        value = value.quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP)
        negative = value < 0
        magnitude = abs(value)
        text = format(magnitude, (',' if grouping else '')+f'.{places}f')
        if not leading and magnitude < 1 and places:
            text = text[1:]
        if profile == 'de-de':
            text = text.translate(str.maketrans('.,', ',.'))
        if kind == 'FormatCurrency':
            text = '$'+text if profile == 'en-us' else text+' €'
        if kind == 'FormatPercent':
            text += '%'
        return '('+text+')' if negative and parentheses else '-'+text if negative else text


def execute(cli, mode, profile, cases, width=5):
    for start in range(0, len(cases), width):
        batch = cases[start:start+width]
        sql = 'SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
        result = subprocess.run([str(cli), 'query', mode, sql, '--number-format', profile], capture_output=True, text=True)
        assert result.returncode == 0, (mode, profile, sql, result.stderr)
        rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
        assert len(rows) == 2 and rows[1] == [spelling(value) for _,value in batch], (mode, profile, batch, rows)
    return len(cases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    with localcontext() as context:
        context.prec = 1024
        values = [Decimal('79228162514264337593543950335'), Decimal('-922337203685477.5808'),
                  Decimal('.005'), Decimal('-.005'), Decimal('0'), Decimal('0.0000000000000000000000000001')]
        values += [Decimal(rng.randrange(-10**28, 10**28)).scaleb(-rng.randrange(29)) for _ in range(64)]
    count = 0
    for mode in ['eval', 'run']:
        for profile in ['invariant', 'en-us', 'de-de']:
            kinds = ['FormatNumber', 'FormatPercent']+(['FormatCurrency'] if profile != 'invariant' else [])
            cases = []
            for value in values:
                token = format(value, 'f').replace('.', ',' if profile == 'de-de' else '.')
                for kind in kinds:
                    places = rng.randrange(9)
                    leading = bool(rng.randrange(2))
                    parentheses = bool(rng.randrange(2))
                    grouping = profile != 'invariant' and bool(rng.randrange(2))
                    expression = f'{kind}(CDec({quoted(token)}),{places},{-int(leading)},{-int(parentheses)},{-int(grouping)})'
                    cases.append((expression, expected(value, kind, profile, places, leading, parentheses, grouping)))
            for kind in kinds:
                cases.append((f'{kind}(-1234.5)', expected(Decimal('-1234.5'), kind, profile)))
                cases.append((f'{kind}(1,,,,)', expected(Decimal(1), kind, profile)))
            count += execute(args.cli, mode, profile, cases)
            float_cases = []
            for value in [0.0, -0.0, .005, -.005, 2.675, .125, 1e20, 1e-20,
                          float.fromhex('0x1.fffffffffffffp+1023'), float.fromhex('0x0.0000000000001p-1022')]:
                normalized = Decimal(format(value, '.14e'))
                token = repr(value).replace('.', ',' if profile == 'de-de' else '.')
                for kind in kinds:
                    float_cases.append((f'{kind}(CDbl({quoted(token)}))', expected(normalized, kind, profile)))
            count += execute(args.cli, mode, profile, float_cases, 1)
            count += execute(args.cli, mode, profile, [('FormatNumber(1,255)', expected(Decimal(1), 'FormatNumber', profile, 255))], 1)
        for expression in ['FormatNumber(NULL)', 'FormatNumber(1,-2)', 'FormatNumber(1,256)',
                           'FormatNumber(1,2,1)', 'FormatNumber(1,NULL)', 'FormatPercent(1,2,0,0,0,1/0)',
                           'FormatCurrency(NULL,1/0)', 'VBA.FormatNumber(1)']:
            result = subprocess.run([str(args.cli), 'query', mode, 'SELECT 1,'+expression, '--number-format', 'en-us'], capture_output=True)
            assert result.returncode != 0 and not result.stdout, expression
            count += 1
    if args.jar:
        assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
        identities = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
        sql = "SELECT ORDR_NBR AS N,FormatNumber(ORDR_NBR) AS V,FormatPercent(ORDR_NBR/100) AS P,FormatCurrency(ORDR_NBR) AS C FROM ROULETTE_TB ORDER BY N"
        for profile in ['en-us', 'de-de']:
            expected_rows = [[str(identity), spelling(expected(Decimal(identity), 'FormatNumber', profile)),
                              spelling(expected(Decimal(identity)/100, 'FormatPercent', profile)),
                              spelling(expected(Decimal(identity), 'FormatCurrency', profile))] for identity in identities]
            for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
                result = subprocess.run([str(args.cli), 'query', 'run', sql, '--database', str(args.database), '--number-format', profile, *options], capture_output=True, text=True)
                assert result.returncode == 0, result.stderr
                rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                assert rows[1:] == expected_rows, (profile, options, rows)
                count += len(expected_rows)*3
        assert digest(args.database) == before
    print(f'Passed {count} independent styled-number/refusal/native-row controls; native Access unrun.')


if __name__ == '__main__':
    main()
