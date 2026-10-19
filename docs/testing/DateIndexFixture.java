import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.math.BigDecimal;
import java.nio.*;
import java.nio.file.*;
import java.util.TimeZone;

/** Original inert DATE index recipe with exact serials and calendar aliases. */
public class DateIndexFixture {
  static Object date(Column column,long bits) throws Exception {
    return ((ColumnImpl)column).read(ByteBuffer.allocate(Long.BYTES).order(ByteOrder.LITTLE_ENDIAN).putLong(bits).array());
  }
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action) throws Exception {
    try {action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    long[] bits={0,0x8000000000000000L,1,0x8000000000000001L,
      Double.doubleToRawLongBits(0.25),Double.doubleToRawLongBits(-0.25),
      Double.doubleToRawLongBits(-1.25),Double.doubleToRawLongBits(-1.75),
      Double.doubleToRawLongBits(1.0000000001),Double.doubleToRawLongBits(-1.0000000001),
      Double.doubleToRawLongBits(45000.123456789),Double.doubleToRawLongBits(-657434.25),
      Double.doubleToRawLongBits(2958465.9999999995)};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.DATE);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table single=new TableBuilder("DateIndexed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME))
        .addIndex(new IndexBuilder("DateAsc").addColumns("Moment"))
        .addIndex(new IndexBuilder("DateDesc").addColumns(false,"Moment")).toTable(db);
      Column moment=single.getColumn("Moment");
      for(int i=0;i<1000;i++)single.addRow(i,i%11==0?null:date(moment,bits[(i*7)%bits.length]));
      for(boolean primary:new boolean[]{false,true}) {
        Table table=new TableBuilder(primary?"DatePrimary":"DateUnique")
          .addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME))
          .addIndex(primary?new IndexBuilder("DateKey").addColumns("Moment").setPrimaryKey()
            :new IndexBuilder("DateKey").addColumns("Moment").setUnique()).toTable(db);
        for(long key:bits) {Object value=date(moment,key);table.addRow(value);rejects(()->table.addRow(value));}
        if(primary)rejects(()->table.addRow((Object)null));
        else {table.addRow((Object)null);table.addRow((Object)null);}
        if(table.getRowCount()!=bits.length+(primary?0:2))throw new AssertionError("DATE physical aliases collapsed");
      }
      Table mixed=new TableBuilder("DateMixed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("A",DataType.SHORT_DATE_TIME)).addColumn(new ColumnBuilder("B",DataType.SHORT_DATE_TIME))
        .addColumn(new ColumnBuilder("G",DataType.GUID)).addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addIndex(new IndexBuilder("MixedAsc").addColumns("A","B","G","Amount","Flag"))
        .addIndex(new IndexBuilder("MixedDirections").addColumns(false,"B","G").addColumns("Flag","A").addColumns(false,"Amount")).toTable(db);
      for(int i=0;i<1000;i++)mixed.addRow(i,i%7==0?null:date(moment,bits[i%bits.length]),
        i%11==0?null:date(moment,bits[(i*5)%bits.length]),
        i%13==0?null:String.format("{%08X-89AB-CDEF-0123-%012X}",i%23,i*17L),
        i%17==0?null:BigDecimal.valueOf(9007199254740993L-i*31L,4),i%2==0);
      TableBuilder ten=new TableBuilder("DateTen").addColumn(new ColumnBuilder("ID",DataType.LONG));
      IndexBuilder key=new IndexBuilder("TenMixed");
      for(int i=0;i<10;i++){ten.addColumn(new ColumnBuilder("K"+i,DataType.SHORT_DATE_TIME));key.addColumns(i%2==0,"K"+i);}
      Table tenTable=ten.addIndex(key).toTable(db);
      for(int i=0;i<500;i++) {
        Object[] row=new Object[11];row[0]=i;
        for(int j=0;j<10;j++)row[j+1]=(i+j)%17==0?null:date(moment,bits[(i+j*3)%bits.length]);
        tenTable.addRow(row);
      }
      new TableBuilder("DateEmpty").addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME))
        .addIndex(new IndexBuilder("EmptyKey").addColumns("Moment")).toTable(db);
    }
    System.out.println("Exact DATE physical aliases, unique/primary and NULL assertions passed.");
  }
}
