import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;

/** Original inert IEEE index recipe, including exact zero/NaN representations. */
public class FloatingIndexFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action) throws Exception {
    try {action.run();} catch(ConstraintViolationException expected) {return;}
    throw new AssertionError("Expected constraint rejection");
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);
    if(Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    int[] singles={0,0x80000000,1,0x80000001,0x007fffff,0x807fffff,0x00800000,0x80800000,
      0x3f800000,0xbf800000,0x7f7fffff,0xff7fffff,0x7f800000,0xff800000,
      0x7fc00001,0xffc00001,0x7fc00002,0xffc00002};
    long[] doubles={0,0x8000000000000000L,1,0x8000000000000001L,0x000fffffffffffffL,
      0x800fffffffffffffL,0x0010000000000000L,0x8010000000000000L,
      0x3ff0000000000000L,0xbff0000000000000L,0x7fefffffffffffffL,0xffefffffffffffffL,
      0x7ff0000000000000L,0xfff0000000000000L,0x7ff8000000000001L,0xfff8000000000001L,
      0x7ff8000000000002L,0xfff8000000000002L};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      for(boolean single:new boolean[]{true,false}) {
        DataType type=single?DataType.FLOAT:DataType.DOUBLE;
        String name=single?"Single":"Double";
        Table table=new TableBuilder(name+"Indexed").addColumn(new ColumnBuilder("ID",DataType.LONG))
          .addColumn(new ColumnBuilder("Value",type))
          .addIndex(new IndexBuilder("FloatAsc").addColumns("Value"))
          .addIndex(new IndexBuilder("FloatDesc").addColumns(false,"Value")).toTable(db);
        for(int i=0;i<1000;i++) {
          Object value=single?(Object)Float.intBitsToFloat(singles[(i*7)%singles.length])
            :(Object)Double.longBitsToDouble(doubles[(i*7)%doubles.length]);
          table.addRow(i,i%11==0?null:value);
        }
        Table finite=new TableBuilder(name+"Finite").addColumn(new ColumnBuilder("ID",DataType.LONG))
          .addColumn(new ColumnBuilder("Value",type))
          .addIndex(new IndexBuilder("FloatAsc").addColumns("Value"))
          .addIndex(new IndexBuilder("FloatDesc").addColumns(false,"Value")).toTable(db);
        for(int i=0;i<1000;i++) {
          Object value=single?(Object)Float.intBitsToFloat(singles[(i*7)%12])
            :(Object)Double.longBitsToDouble(doubles[(i*7)%12]);
          finite.addRow(i,i%11==0?null:value);
        }
        Table unique=new TableBuilder(name+"Unique").addColumn(new ColumnBuilder("Value",type))
          .addIndex(new IndexBuilder("UniqueKey").addColumns("Value").setUnique()).toTable(db);
        for(int i=0;i<singles.length;i++) {
          Object value=single?(Object)Float.intBitsToFloat(singles[i]):(Object)Double.longBitsToDouble(doubles[i]);
          unique.addRow(value);rejects(()->unique.addRow(value));
        }
        unique.addRow((Object)null);unique.addRow((Object)null);
        // Both zeros and distinct NaN payloads are accepted as different physical keys.
        if(unique.getRowCount()!=20) throw new AssertionError("Distinct IEEE keys");
      }
      Table mixed=new TableBuilder("FloatingMixed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("S",DataType.FLOAT)).addColumn(new ColumnBuilder("D",DataType.DOUBLE))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY)).addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("G",DataType.GUID))
        .addIndex(new IndexBuilder("MixedAsc").addColumns("S","D","Amount","Flag","G"))
        .addIndex(new IndexBuilder("MixedDirections").addColumns(false,"D","G")
          .addColumns("Flag","S").addColumns(false,"Amount")).toTable(db);
      for(int i=0;i<1000;i++) mixed.addRow(i,i%7==0?null:Float.intBitsToFloat(singles[i%singles.length]),
        i%11==0?null:Double.longBitsToDouble(doubles[(i*5)%doubles.length]),
        i%13==0?null:BigDecimal.valueOf(9007199254740993L-i*31L,4),i%2==0,
        i%17==0?null:String.format("{%08X-89AB-CDEF-0123-%012X}",i%23,i*17L));
      TableBuilder ten=new TableBuilder("FloatingTen").addColumn(new ColumnBuilder("ID",DataType.LONG));
      IndexBuilder key=new IndexBuilder("TenMixed");
      for(int i=0;i<10;i++) {ten.addColumn(new ColumnBuilder("K"+i,DataType.DOUBLE));key.addColumns(i%2==0,"K"+i);}
      Table tenTable=ten.addIndex(key).toTable(db);
      for(int i=0;i<500;i++) {
        Object[] row=new Object[11];row[0]=i;
        for(int j=0;j<10;j++) row[j+1]=(i+j)%17==0?null:Double.longBitsToDouble(doubles[(i+j*3)%doubles.length]);
        tenTable.addRow(row);
      }
      new TableBuilder("FloatingEmpty").addColumn(new ColumnBuilder("Value",DataType.DOUBLE))
        .addIndex(new IndexBuilder("EmptyKey").addColumns("Value")).toTable(db);
    }
    System.out.println("Exact IEEE, signed-zero/NaN uniqueness and repeated NULL assertions passed.");
  }
}
