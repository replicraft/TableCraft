"""Independent Gregorian/Automation DATE conversion and component-projection models.

Optional native rows supply input through Jackcess; native Access remains unrun.
"""
import argparse
import csv
import datetime as dt
from io import StringIO
import math
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest
from compare_string_search import quoted

EPOCH = dt.date(1899,12,30)


def serial(value):
    day=(value.date()-EPOCH).days
    fraction=(value.hour*3600+value.minute*60+value.second)/86400
    return day-fraction if day<0 else day+fraction


def display(value):
    day=math.trunc(value)
    nanos=round(abs(value-day)*(86400*10**9))
    carry,nanos=divmod(nanos,86400*10**9)
    date=EPOCH+dt.timedelta(days=day+carry)
    seconds,fraction=divmod(nanos,10**9)
    hour,seconds=divmod(seconds,3600)
    minute,second=divmod(seconds,60)
    return f'#{date.year:04}-{date.month:02}-{date.day:02} {hour:02}:{minute:02}:{second:02}.{fraction:09}#'


def spelling(value,profile):
    if profile=='iso':
        return f'{value.year:04}-{value.month:02}-{value.day:02} {value.hour:02}:{value.minute:02}:{value.second:02}'
    if profile=='en-us':
        return f'{value.month}/{value.day}/{value.year:04} {value.hour%12 or 12}:{value.minute:02}:{value.second:02} '+('AM' if value.hour<12 else 'PM')
    return f'{value.day}.{value.month}.{value.year:04} {value.hour:02}:{value.minute:02}:{value.second:02}'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli',type=Path,required=True)
    parser.add_argument('--jar',type=Path)
    parser.add_argument('--database',type=Path)
    args=parser.parse_args()
    if bool(args.jar)!=bool(args.database):
        parser.error('--jar and --database must be supplied together')
    rng=random.Random(20261020)
    values=[dt.datetime(100,1,1,6),dt.datetime(9999,12,31,23,59,59),
            dt.datetime(1899,12,29,6),dt.datetime(1899,12,30,6),dt.datetime(1969,2,12,16,35,17)]
    values += [dt.datetime(1900,1,1)+dt.timedelta(days=rng.randrange(70000),seconds=rng.randrange(86400)) for _ in range(128)]
    count=0
    for mode in ['eval','run']:
        for profile in ['iso','en-us','de-de']:
            controls=[]
            for value in values:
                text=spelling(value,profile)
                full=serial(value)
                date=float((value.date()-EPOCH).days)
                time=(value.hour*3600+value.minute*60+value.second)/86400
                controls += [(f'CDate({quoted(text)})',display(full)),
                             (f'DateValue({quoted(text)})',display(date)),
                             (f'TimeValue({quoted(text)})',display(time))]
            controls += [("DateValue('February 12, 1969')",display(25246.0)),
                         ("DateValue('12-Feb-69')",display(25246.0)),
                         ("TimeValue('2:24PM')",display((14*3600+24*60)/86400)),
                         ('DateValue(NULL)','NULL'),('TimeValue(NULL)','NULL')]
            for start in range(0,len(controls),4):
                batch=controls[start:start+4]
                sql='SELECT '+','.join(f'{expression} AS C{i}' for i,(expression,_) in enumerate(batch))
                result=subprocess.run([str(args.cli),'query',mode,sql,'--date-format',profile],capture_output=True,text=True)
                assert result.returncode==0,(profile,sql,result.stderr)
                rows=list(csv.reader(StringIO(result.stdout),delimiter='\t'))
                assert rows[1]==[expected for _,expected in batch],(mode,profile,batch,rows)
                count+=len(batch)
            result=subprocess.run([str(args.cli),'query',mode,"SELECT Year(DateValue('Feb 29')) AS Y",'--date-format',profile,'--reference-year','2024'],capture_output=True,check=True,text=True)
            assert list(csv.reader(StringIO(result.stdout),delimiter='\t'))[1]==['2024']
            count+=1
        for number in [-0.25,-1.25,0.25,1.25,45000.125]:
            sql=f'SELECT CDate({number!r}) AS D,DateValue({number!r}) AS V,TimeValue({number!r}) AS T'
            result=subprocess.run([str(args.cli),'query',mode,sql],capture_output=True,check=True,text=True)
            assert list(csv.reader(StringIO(result.stdout),delimiter='\t'))[1]==[display(number),display(float(math.trunc(number))),display(abs(number-math.trunc(number)))]
            count+=3
        for expression in ["DateValue('2026-01-01 89:98')","TimeValue('2026-02-30 12:00')","CDate(NULL)","DateValue('Feb 29')","CDate(1,1/0)"]:
            result=subprocess.run([str(args.cli),'query',mode,'SELECT 1,'+expression,'--date-format','iso'],capture_output=True)
            assert result.returncode!=0 and not result.stdout,expression
            count+=1
    if args.jar:
        assert digest(args.jar)=='19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
        before=digest(args.database)
        tagged=run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
        rows=[]
        for line in tagged.splitlines():
            cells=line.split('\t')[1:]
            identity=int(cells[0].removeprefix('I32:'))
            number=float(bytes.fromhex(cells[1].removeprefix('T')).decode('utf-8'))
            rows.append([str(identity),display(float(identity)),display(number),display(0.0)])
        rows.sort(key=lambda row:int(row[0]))
        sql='SELECT ORDR_NBR AS N,CDate(ORDR_NBR) AS D,DateValue(CDate(NBR_VALUE)) AS V,TimeValue(CDate(ORDR_NBR)) AS T FROM ROULETTE_TB ORDER BY N'
        for options in [[],['--index','ROULETTE_TB','Number']]:
            result=subprocess.run([str(args.cli),'query','run',sql,'--database',str(args.database),'--date-format','iso','--number-format','invariant',*options],capture_output=True,check=True,text=True)
            assert list(csv.reader(StringIO(result.stdout),delimiter='\t'))[1:]==rows
            count+=len(rows)*3
        assert digest(args.database)==before
    print(f'Passed {count} independent date-conversion/refusal/native-row controls; native Access unrun.')


if __name__=='__main__':
    main()
