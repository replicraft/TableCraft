"""Compare exact Boolean/IEEE/DATE index updates and complete preserved spans."""
import argparse
from pathlib import Path
from compare_batch_updates import Cell, compare


def b(table,row,old,new,column="Flag"):
    return Cell(table,column,row,"boolean",old,new)

def f(table,row,old,new,single=False):
    return Cell(table,"Value",row,"single-bits" if single else "double-bits",old,new)

def d(table,row,old,new,column="Moment"):
    return Cell(table,column,row,"date-bits",old,new)

BOOLEAN=(
    (b("BooleanUnique",0,True,False),b("BooleanUnique",1,False,True)),
    (b("BooleanPrimary",0,True,False),b("BooleanPrimary",1,False,True)),
    (b("BooleanIndexed",1,False,True),b("BooleanIndexed",3,True,False)),
    (b("BooleanIndexed",0,False,False),),
    (b("BooleanMixed",1,False,True),b("BooleanMixed",1,False,True,"Other")),
    (b("BooleanTupleUnique",0,True,False),b("BooleanTupleUnique",1,False,True)),
)
BOOLEAN_INVALID=(
    ((b("BooleanPrimary",0,True,False),),b"DuplicateKey"),
    ((b("BooleanUnique",0,True,None),),b"Column"),
    ((b("BooleanUnique",0,False,True),),b"Conflict"),
)
FLOATING=(
    (f("SingleUnique",0,"00000000","80000000",True),f("SingleUnique",1,"80000000","00000000",True)),
    (f("DoubleUnique",14,"7ff8000000000001","7ff8000000000002"),f("DoubleUnique",16,"7ff8000000000002","7ff8000000000001")),
    (f("SingleUnique",14,"7fc00001","7fc00003",True),),
    (f("DoubleUnique",15,"fff8000000000001","fff8000000000003"),),
    (f("SingleUnique",18,None,"3f800001",True),),
    (f("DoubleUnique",8,"3ff0000000000000",None),),
    (f("SingleIndexed",1,"80800000","80800000",True),),
    (f("DoubleIndexed",1,"8010000000000000","8010000000000001"),),
    (Cell("FloatingMixed","S",1,"single-bits","80000000","80000001"),
     Cell("FloatingMixed","D",1,"double-bits","800fffffffffffff","8010000000000001"),
     b("FloatingMixed",1,False,True)),
)
FLOATING_INVALID=(
    ((f("SingleUnique",0,"00000000","80000000",True),),b"DuplicateKey"),
    ((f("DoubleUnique",0,"0000000000000000","0000000000000001"),),b"DuplicateKey"),
    ((f("DoubleUnique",0,"8000000000000000","0000000000000001"),),b"Conflict"),
)
DATES=(
    (d("DateUnique",0,"0000000000000000","8000000000000000"),d("DateUnique",1,"8000000000000000","0000000000000000")),
    (d("DateUnique",4,"3fd0000000000000","bfd0000000000000"),d("DateUnique",5,"bfd0000000000000","3fd0000000000000")),
    (d("DateUnique",8,"3ff000000006df38","3ff0000000000001"),),
    (d("DateUnique",13,None,"4004000000000000"),),
    (d("DateUnique",4,"3fd0000000000000",None),),
    (d("DateIndexed",1,"bffc000000000000","bffc000000000001"),),
    (d("DateIndexed",0,None,None),),
    (d("DateMixed",1,"8000000000000000","3ff0000000000001","A"),
     d("DateMixed",1,"bfd0000000000000","bfd0000000000001","B"),b("DateMixed",1,False,True)),
)
DATE_INVALID=(
    ((d("DatePrimary",0,"0000000000000000",None),),b"NullKey"),
    ((d("DateUnique",0,"0000000000000000","8000000000000000"),),b"DuplicateKey"),
    ((d("DateUnique",0,"8000000000000000","4004000000000000"),),b"Conflict"),
)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("jar","cli","rows","preview","checker","boolean2016","boolean2019","floating2016","floating2019","date2016","date2019"):
        parser.add_argument("--"+name,required=True,type=Path)
    args=parser.parse_args()
    compare(args,((args.boolean2016,5,BOOLEAN,BOOLEAN_INVALID),(args.boolean2019,6,BOOLEAN,BOOLEAN_INVALID),
                  (args.floating2016,5,FLOATING,FLOATING_INVALID),(args.floating2019,6,FLOATING,FLOATING_INVALID),
                  (args.date2016,5,DATES,DATE_INVALID),(args.date2019,6,DATES,DATE_INVALID)))

if __name__=="__main__":
    main()
