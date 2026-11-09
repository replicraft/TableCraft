"""Compare Decimal slot/index changes, original-association cascades and preservation."""
import argparse
from pathlib import Path
from compare_batch_updates import Cell, compare

G="00112233-4455-6677-8899-aabbccddeeff"
NEW="fedcba98-7654-3210-ffff-000011112222"
MAX="9999999999999999999999999999"
def d(table,column,row,old,new):return Cell(table,column,row,"decimal",old,new)
def amount(table,row,old,new):return d(table,"Amount",row,old,new)
def fixed(table,column,row,kind,old,new):return Cell(table,column,row,kind,old,new)

CASES=(
    (amount("Plain",0,"1:2","9999:2"),),
    (amount("Plain",0,"1:2",None),),
    (amount("Plain",1,None,None),),
    (amount("Plain",0,"1:2","1:2"),),
    (amount("Tree",1,"-14:4","-13:4"),),
    (amount("Parents",0,"1:4","-9007199254740993:4"),amount("Children",0,"1:4","-9007199254740993:4"),amount("Children",4,"1:4","-9007199254740993:4")),
    (amount("Parents",0,"1:4",None),amount("Children",0,"1:4",None),amount("Children",4,"1:4",None)),
    tuple(c for table,rows in (("TupleParents",(0,)),("TupleChildren",(0,4))) for row in rows for c in (
        amount(table,row,"1:4","-"+MAX+":4"),d(table,"Detail",row,"1:28",MAX+":28"),
        fixed(table,"Band",row,"int64",0,9223372036854775807),fixed(table,"G",row,"guid",G,NEW))),
    tuple(c for table,rows in (("TupleParents",(0,)),("TupleChildren",(0,4))) for row in rows for c in (
        amount(table,row,"1:4",None),d(table,"Detail",row,"1:28",None),
        fixed(table,"Band",row,"int64",0,None),fixed(table,"G",row,"guid",G,None))),
    (amount("Parents",0,"1:4","2:4"),amount("Parents",1,"2:4","1:4"),amount("Children",0,"1:4","2:4"),amount("Children",1,"2:4","1:4"),amount("Children",4,"1:4","2:4")),
    (d("Plain","Detail",0,"-1:28",MAX+":28"),),
)
INVALID=(
    ((amount("Plain",0,"1:2","10000:2"),),b"Malformed"),
    ((amount("Plain",0,"1:2","1:1"),),b"Malformed"),
    ((amount("Plain",1,None,"1:2"),),b"RowLayout"),
    ((amount("Parents",0,"1:4","2:4"),),b"DuplicateKey"),
    ((amount("Children",0,"1:4","9:4"),),b"MissingParent"),
    ((amount("Parents",0,"2:4","9:4"),),b"Conflict"),
    ((amount("Plain",0,"1:2","2:2"),amount("Plain",0,"1:2","2:2")),b"Conflict"),
)
def request(cells):
    parents=tuple(c for c in cells if c.table in ("Parents","TupleParents"))
    return parents or cells

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("jar","cli","rows","preview","checker","ace2016","ace2019"):
        parser.add_argument("--"+name,required=True,type=Path)
    args=parser.parse_args()
    compare(args,((args.ace2016,5,CASES,INVALID),(args.ace2019,6,CASES,INVALID)),request=request,cascade=True)
if __name__=="__main__":main()
