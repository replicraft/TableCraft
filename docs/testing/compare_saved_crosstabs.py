"""Independent native crosstab assembly/getter/model checks, not Access execution.

Dynamic heading discovery follows TableCraft's current explicit profile, including
its unfiltered discovery domain. Windows Access qualification remains pending.
"""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES, display
from compare_saved_queries import catalog_ids
from compare_composite_constraints import roots
from compare_complex_links import column_layout
from compare_attachment_columns import fields

PROFILES = [('V2000','jet4.mdb'),('V2007','2007.accdb'),('V2016','2016.accdb'),('V2019','2019.accdb')]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    parser.add_argument('--top', action='store_true', help='Use the extended stored TOP profile')
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-cross-oracle-') as classes:
        subprocess.run(['javac','-cp',str(args.jar),'-d',classes,*[str(RECIPES/(name+'.java')) for name in ['SavedCrosstabFixture','SavedReadOracle','SavedQueryOracle','TableDataOracle']]],check=True)
        classpath = str(args.jar)+os.pathsep+classes
        def java(name,*values):
            return subprocess.check_output(['java','-Dstdout.encoding=UTF-8','-cp',classpath,name,*map(str,values)],text=True,encoding='utf-8')
        def cli(*values,accepted=True):
            result=subprocess.run([str(args.cli),*map(str,values)],capture_output=True,text=True,encoding='utf-8')
            if accepted:
                assert result.returncode==0,(values,result.stderr)
            else:
                assert result.returncode==2 and not result.stdout,(values,result)
            return result.stdout
        def rows(text):
            return list(csv.reader(StringIO(text),delimiter='\t'))
        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile,suffix in PROFILES:
                java('SavedCrosstabFixture',args.directory/('cross-'+suffix),profile,*(['--top'] if args.top else []))
        for _,suffix in PROFILES:
            path=args.directory/('cross-'+suffix)
            before=path.read_bytes()
            names=java('SavedReadOracle',path).splitlines()
            assert len(names)==(20 if args.top else 14)
            inputs=[]
            for line in java('TableDataOracle',path,'Sales').splitlines():
                table,*values=line.split('\t');assert table=='Sales'
                inputs.append([None if value=='N' else int(display(value)) for value in values])
            assert inputs==[[1,2,10],[1,1,3],[1,2,7],[2,3,5],[2,1,None],[3,None,8]]
            def total(records):
                values=[record[2] for record in records if record[2] is not None]
                return sum(values) if values else None
            def expected(name):
                if name=='LocalSales':
                    return [['Region','Quarter','Amount'],*[[str(value) if value is not None else 'NULL' for value in record] for record in inputs]]
                headings=[3,2,4] if name=='CrossFixed' else ['B','A','C'] if name=='CrossText' else [1,2,3]
                def pivot(record):
                    return ('B' if record[1]==2 else 'A') if name=='CrossText' else record[1]
                selected=[record for record in inputs if (name!='CrossFiltered' or record[1]==2) and (name!='CrossParameter' or record[0]>=2)]
                result=[['Region','Total',*map(str,headings)]]
                for key in sorted({record[0] for record in selected}):
                    group=[record for record in selected if record[0]==key]
                    group_total=total(group)
                    if (name=='CrossHaving' and not(group_total is not None and group_total>10)) or (name=='CrossTopHaving' and not(group_total is not None and group_total<10)):
                        continue
                    cells=[]
                    for heading in headings:
                        cell=[record for record in group if pivot(record)==heading]
                        cells.append(len(cell) if name=='CrossCount' and cell else total(cell))
                    result.append([str(value) if value is not None else 'NULL' for value in [key,group_total,*cells]])
                if name in {'CrossHiddenTop','CrossTopComposite'}: result=result[:2]
                if name=='CrossTopPercent': result=result[:3]
                if name=='CrossTopHaving': result=result[:2]
                if name=='CrossTopTies': result=[result[0],*[row for row in result[1:] if int(row[0])%2==1]]
                return result
            refused={'CrossOwner','CrossUdf','CrossTopError','CrossTopZero'}
            for name in names:
                raw=java('SavedQueryOracle',path,name)
                assert rows(cli('inspect',path,'--query',name))==[line.split('\t') for line in raw.splitlines()]
                controls+=1
                sql=java('SavedReadOracle',path,name)
                flag=[line.split('\t') for line in raw.splitlines()[5:] if line.startswith('3\t')]
                assert len(flag)==1
                if int(flag[0][2]) & 16:
                    # The renderer omits stored TOP: the independent raw getter
                    # controls count/PERCENT retention in expected owned syntax.
                    count=flag[0][4].strip('"');assert count.isascii() and count.isdigit()
                    assert 'TOP' not in sql
                    sql=sql.replace('SELECT ',f'SELECT TOP {count} '+('PERCENT ' if int(flag[0][2])&32 else ''),1)
                assert cli('query','saved','--database',path,name)==cli('query','parse',sql),name
                controls+=1
                params=['--param','Floor=long:2'] if name=='CrossParameter' else []
                for query in [sql,None]:
                    command=['query','run']+(['--saved',name] if query is None else [query])+['--database',path,'--collation','ordinal',*params]
                    actual=cli(*command,accepted=name not in refused)
                    if name not in refused:
                        assert rows(actual)==expected(name),(name,query,rows(actual),expected(name))
                    controls+=1
                # Native named crosstabs do not invent a fixed schema or scan user
                # records during binding; the ordinary SELECT dependency works.
                actual=cli('query','run',f'SELECT * FROM [{name}]','--database',path,accepted=name=='LocalSales')
                if name=='LocalSales':assert rows(actual)==expected(name)
                controls+=1
            for params in [[],['--param','Floor=text:2']]:
                cli('query','run','--database',path,'--saved','CrossParameter',*params,accepted=False)
                controls+=1
            ids=catalog_ids(args,path);definition=ids['MSysQueries']
            object_offset,_,_=column_layout(before,definition,'ObjectId')
            layout=fields(before,definition)
            def fixed_offset(name,width):
                at=layout[name][0];assert before[at+15]&1
                assert int.from_bytes(before[at+23:at+25],'little')==width
                return int.from_bytes(before[at+21:at+23],'little')
            attribute_offset=fixed_offset('Attribute',1);flag_offset=fixed_offset('Flag',2)
            selected=[slot for slot in roots(before,definition) if int.from_bytes(before[slot[0]+2+object_offset:slot[0]+6+object_offset],'little',signed=True)==ids['CrossDynamic']]
            positions={}
            for slot in selected:positions.setdefault(before[slot[0]+2+attribute_offset],[]).append(slot[0])
            for attribute,index,value in [(1,0,1),(6,0,3),(9,0,0),(3,0,64)]:
                changed=bytearray(before);start=positions[attribute][index]+2+flag_offset
                changed[start:start+2]=value.to_bytes(2,'little')
                with tempfile.TemporaryDirectory(prefix='tablecraft-cross-refusal-') as temporary:
                    copy=Path(temporary)/path.name;copy.write_bytes(changed)
                    for command in [['query','saved','--database',copy,'CrossDynamic'],['query','run','--database',copy,'--saved','CrossDynamic']]:
                        cli(*command,accepted=False);controls+=1
                    assert copy.read_bytes()==changed
            assert path.read_bytes()==before
            print(path.name,len(before),hashlib.sha256(before).hexdigest())
    print(f'Passed {controls} saved crosstab assembly/execution/getter/refusal controls; native Access unrun.')


if __name__=='__main__':
    main()
