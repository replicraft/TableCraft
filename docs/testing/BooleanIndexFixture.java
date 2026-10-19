import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;

/** Original inert Yes/No index recipe; writes only fresh disposable databases. */
public class BooleanIndexFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action) throws Exception {
    try {action.run();} catch(ConstraintViolationException expected) {return;}
    throw new AssertionError("Expected unique constraint rejection");
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
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table single=new TableBuilder("BooleanIndexed")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addIndex(new IndexBuilder("BoolAsc").addColumns("Flag"))
        .addIndex(new IndexBuilder("BoolDesc").addColumns(false,"Flag")).toTable(db);
      for(int i=0;i<1000;i++) single.addRow(i,i%7==0?null:i%3==0);
      // NULL inputs become stored false; neither the row nor index gains NULL.
      if(!Boolean.FALSE.equals(single.iterator().next().get("Flag"))) throw new AssertionError("NULL conversion");
      for(boolean primary:new boolean[]{false,true}) {
        Table unique=new TableBuilder(primary?"BooleanPrimary":"BooleanUnique")
          .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
          .addIndex(primary?new IndexBuilder("BoolKey").addColumns("Flag").setPrimaryKey()
            :new IndexBuilder("BoolKey").addColumns("Flag").setUnique()).toTable(db);
        unique.addRow(true);unique.addRow(false);
        rejects(()->unique.addRow(true));rejects(()->unique.addRow((Object)null));
      }
      Table mixed=new TableBuilder("BooleanMixed")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addColumn(new ColumnBuilder("G",DataType.GUID))
        .addColumn(new ColumnBuilder("Small",DataType.INT))
        .addColumn(new ColumnBuilder("Other",DataType.BOOLEAN))
        .addIndex(new IndexBuilder("MixedAsc").addColumns("Flag","Amount","G","Small","Other"))
        .addIndex(new IndexBuilder("MixedDirections").addColumns(false,"Other","G")
          .addColumns("Amount","Flag").addColumns(false,"Small")).toTable(db);
      for(int i=0;i<1000;i++) mixed.addRow(i,i%5==0?null:i%2==0,
        i%11==0?null:BigDecimal.valueOf(9007199254740993L-i*17L,4),
        i%13==0?null:String.format("{%08X-89AB-CDEF-0123-%012X}",i%29,i*31L),
        i%17==0?null:(short)(i%101-50),i%3==0);
      TableBuilder ten=new TableBuilder("BooleanTen").addColumn(new ColumnBuilder("ID",DataType.LONG));
      IndexBuilder tenKey=new IndexBuilder("TenMixed");
      for(int i=0;i<10;i++) {
        ten.addColumn(new ColumnBuilder("K"+i,DataType.BOOLEAN));
        tenKey.addColumns(i%2==0,"K"+i);
      }
      Table tenTable=ten.addIndex(tenKey).toTable(db);
      for(int i=0;i<500;i++) {
        Object[] row=new Object[11];row[0]=i;
        for(int j=0;j<10;j++) row[j+1]=((i>>j)&1)!=0;
        tenTable.addRow(row);
      }
      Table tuple=new TableBuilder("BooleanTupleUnique")
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addIndex(new IndexBuilder("TupleKey").addColumns("Flag","Amount").setUnique()).toTable(db);
      tuple.addRow(true,null);tuple.addRow(false,null);tuple.addRow(true,BigDecimal.ZERO);
      tuple.addRow(false,BigDecimal.ZERO);tuple.addRow(false,BigDecimal.valueOf(1,4));
      rejects(()->tuple.addRow(true,null));rejects(()->tuple.addRow(null,null));
      new TableBuilder("BooleanEmpty").addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addIndex(new IndexBuilder("EmptyKey").addColumns("Flag")).toTable(db);
    }
    System.out.println("Boolean conversion and unique/primary/partial-NULL assertions passed.");
  }
}
