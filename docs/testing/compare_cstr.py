"""Check CStr's explicit numeric/date profiles against independent Python models.

Optional native rows supply input through Jackcess; native Access is unrun.
"""
import argparse
import csv
import datetime as dt
from decimal import Decimal
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest
from compare_numeric_text import floating
from compare_string_search import quoted


def spelling(text):
    return display('T'+text.encode('utf-8').hex())


def date_text(value, profile):
    if value.microsecond >= 500000:
        value += dt.timedelta(seconds=1)
    value = value.replace(microsecond=0)
    show_date = value.date() != dt.date(1899, 12, 30)
    show_time = not show_date or value.time() != dt.time()
    date = ''
    if show_date:
        if profile == 'iso':
            date = f'{value.year:04}-{value.month:02}-{value.day:02}'
        elif profile == 'en-us':
            date = f'{value.month}/{value.day}/{value.year:04}'
        else:
            date = f'{value.day:02}.{value.month:02}.{value.year:04}'
    time = ''
    if show_time:
        if profile == 'en-us':
            time = f'{value.hour%12 or 12}:{value.minute:02}:{value.second:02} '+('AM' if value.hour < 12 else 'PM')
        else:
            time = f'{value.hour:02}:{value.minute:02}:{value.second:02}'
    return date+(' ' if date and time else '')+time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--database', type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng = random.Random(20261020)
    numeric = [(f'CStr({value})', str(value)) for value in
               [0, -(2**63), 2**63-1]+[rng.randrange(-(2**63), 2**63) for _ in range(256)]]
    for _ in range(128):
        coefficient = rng.randrange(-10**16, 10**16)
        value = Decimal(coefficient).scaleb(-4)
        text = format(value, 'f')
        expected = text.rstrip('0').rstrip('.')
        numeric.append((f"CStr(CCur('{text}'))", expected))
    for text in ['79228162514264337593543950335','0.0000000000000000000000000001','1.2300']:
        expected=text.rstrip('0').rstrip('.') if '.' in text else text
        numeric.append((f"CStr(CDec('{text}'))",expected))
    values = [0.0, -0.0, .1, -.5, 459.001, 1.2345678901234567, 1e15, 1e-300,
              float.fromhex('0x0.0000000000001p-1022'), float.fromhex('0x1.fffffffffffffp+1023')]
    floats = [(f'CStr(CDbl({value!r}))', floating(value, 15).lstrip(' ')) for value in values]
    controls = 0
    for mode in ['eval', 'run']:
        for profile in ['invariant', 'en-us', 'de-de']:
            for cases, width in [(numeric, 8), (floats, 1)]:
                for start in range(0, len(cases), width):
                    batch = cases[start:start+width]
                    sql = 'SELECT '+','.join(f'{(expression.replace(".", ",") if profile == "de-de" and expression.startswith(("CStr(CCur(","CStr(CDec(")) else expression)} AS C{i}' for i,(expression,_) in enumerate(batch))
                    result = subprocess.run([str(args.cli), 'query', mode, sql, '--number-format', profile], capture_output=True, text=True)
                    assert result.returncode == 0, (mode, profile, sql, result.stderr)
                    rows = list(csv.reader(StringIO(result.stdout), delimiter='\t'))
                    expected = [spelling(text.replace('.', ',') if profile == 'de-de' else text) for _,text in batch]
                    assert rows[1] == expected, (mode, profile, batch, rows)
                    controls += len(batch)
        dates = [dt.datetime(1899,12,30),dt.datetime(1899,12,30,6),dt.datetime(2026,10,20),
                 dt.datetime(2026,10,20,13,4,5),dt.datetime(100,1,1),dt.datetime(9999,12,31),
                 dt.datetime(2024,2,29,23,59,59,750000)]
        dates += [dt.datetime(1900,1,1)+dt.timedelta(days=rng.randrange(70000),seconds=rng.randrange(86400),microseconds=rng.choice([0,250000,750000])) for _ in range(128)]
        for profile in ['iso', 'en-us', 'de-de']:
            for start in range(0, len(dates), 8):
                batch=dates[start:start+8]
                sql='SELECT '+','.join(f'CStr(#{value.year:04}-{value.month:02}-{value.day:02} {value.hour:02}:{value.minute:02}:{value.second:02}.{value.microsecond:06}#) AS C{i}' for i,value in enumerate(batch))
                result=subprocess.run([str(args.cli),'query',mode,sql,'--date-format',profile],capture_output=True,text=True)
                assert result.returncode==0,(mode,profile,sql,result.stderr)
                rows=list(csv.reader(StringIO(result.stdout),delimiter='\t'))
                assert rows[1]==[spelling(date_text(value,profile)) for value in batch],(profile,batch,rows)
                controls+=len(batch)
        for value in ['', '1.23', 'é😀\0\n"']:
            assert run(args.cli,'query',mode,'SELECT CStr('+quoted(value).replace('\0', "' & Chr(0) & '")+') AS T')=='T\n'+spelling(value)+'\n'
            controls+=1
        for expression in ['CStr(NULL)','CStr(1)','CStr(#2026-10-20#)',"CStr('x',1/0)"]:
            result=subprocess.run([str(args.cli),'query',mode,'SELECT 1,'+expression],capture_output=True)
            assert result.returncode!=0 and not result.stdout,expression
            controls+=1
    if args.jar:
        assert digest(args.jar)=='19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before=digest(args.database)
        tagged=run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
        rows=[]
        for line in tagged.splitlines():
            cells=line.split('\t')[1:]
            identity=int(cells[0].removeprefix('I32:'))
            text=bytes.fromhex(cells[1].removeprefix('T')).decode('utf-8')
            rows.append((identity,text))
        rows.sort()
        sql='SELECT ORDR_NBR AS N,CStr(ORDR_NBR) AS S,CStr(NBR_VALUE) AS T,CStr(DateSerial(2000+ORDR_NBR,2,28)) AS D FROM ROULETTE_TB ORDER BY N'
        for options in [[],['--index','ROULETTE_TB','Number']]:
            for number,date in [('invariant','iso'),('en-us','de-de'),('de-de','en-us')]:
                result=subprocess.run([str(args.cli),'query','run',sql,'--database',str(args.database),'--number-format',number,'--date-format',date,*options],capture_output=True,check=True,text=True)
                actual=list(csv.reader(StringIO(result.stdout),delimiter='\t'))[1:]
                expected=[[str(n),spelling(str(n)),spelling(text),spelling(date_text(dt.datetime(2000+n,2,28),date))] for n,text in rows]
                assert actual==expected,(number,date)
                controls+=len(rows)*3
        assert digest(args.database)==before
    print(f'Passed {controls} independent CStr/numeric/date/refusal/native-row controls; native Access unrun.')


if __name__=='__main__':
    main()
