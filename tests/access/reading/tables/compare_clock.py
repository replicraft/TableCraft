"""Check host-issued Date/Time/Now readings with independent Gregorian models.

Fixed local clock overrides are deterministic; native Windows Access is unrun.
"""
import argparse
import csv
import datetime as dt
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
    values = [dt.datetime(100,1,1),dt.datetime(9999,12,31,23,59,59),
              dt.datetime(1899,12,29,6),dt.datetime(1899,12,30),dt.datetime(2024,2,29,23,59,59),
              dt.datetime(1969,2,12,16,35,17),dt.datetime(2026,3,8,2,30),dt.datetime(2026,11,1,1,30)]
    values += [dt.datetime(1900,1,1)+dt.timedelta(days=rng.randrange(73000),seconds=rng.randrange(86400)) for _ in range(128)]
    sql = "SELECT Year(Now()),Month(Date()),Day(Date()),Hour(Time()),Minute(Now()),Second(Time()),Hour(Date()),Year(Time()),DateDiff('s',Date(),Now())"
    count = 0
    for mode in ['eval','run']:
        for value in values:
            text = f'{value.year:04}-{value.month:02}-{value.day:02}T{value.hour:02}:{value.minute:02}:{value.second:02}.{rng.randrange(10**9):09}'
            result = subprocess.run([str(args.cli),'query',mode,sql,'--clock',text],capture_output=True,text=True)
            assert result.returncode == 0,(mode,text,result.stderr)
            rows = list(csv.reader(StringIO(result.stdout),delimiter='\t'))
            expected = [value.year,value.month,value.day,value.hour,value.minute,value.second,0,1899,value.hour*3600+value.minute*60+value.second]
            assert len(rows)==2 and rows[1]==list(map(str,expected)),(mode,text,rows,expected)
            count += len(expected)
        for expression in ['Now(1/0)','Date(*)','Time(,)','VBA.Now()']:
            result = subprocess.run([str(args.cli),'query',mode,'SELECT 1,'+expression,'--clock','2026-10-20T12:34:56'],capture_output=True)
            assert result.returncode != 0 and not result.stdout,expression
            count += 1
    if args.jar:
        assert digest(args.jar)=='19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before = digest(args.database)
        tagged = run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
        identities = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
        expected = [[str(identity),'59','2024','23'] for identity in identities]
        sql = "SELECT ORDR_NBR AS N,DateDiff('d',#2024-01-01#,Date()) AS D,Year(Now()) AS Y,Hour(Time()) AS H FROM ROULETTE_TB ORDER BY N"
        for options in [[],['--index','ROULETTE_TB','Number']]:
            result = subprocess.run([str(args.cli),'query','run',sql,'--database',str(args.database),'--clock','2024-02-29T23:59:59.999999999',*options],capture_output=True,text=True)
            assert result.returncode == 0,result.stderr
            rows = list(csv.reader(StringIO(result.stdout),delimiter='\t'))
            assert rows[1:]==expected,(options,rows)
            count += len(expected)*3
        assert digest(args.database)==before
    print(f'Passed {count} independent clock/refusal/native-row controls; native Access unrun.')


if __name__=='__main__':
    main()
