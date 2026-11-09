import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.*;
import java.nio.file.*;
import java.util.TimeZone;

/** Original inert exact DATE carrier recipe, with deliberate precision probes. */
public class DateBitsFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    long[] bits={0,0x8000000000000000L,1,0x8000000000000001L,
      Double.doubleToRawLongBits(0.25),Double.doubleToRawLongBits(-0.25),
      Double.doubleToRawLongBits(-1.25),Double.doubleToRawLongBits(-1.75),
      Double.doubleToRawLongBits(1.0000000001),Double.doubleToRawLongBits(-1.0000000001),
      Double.doubleToRawLongBits(45000.123456789),Double.doubleToRawLongBits(-657434.25),
      Double.doubleToRawLongBits(2958465.9999999995)};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.DATE);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table table=new TableBuilder("ExactDates").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Value",DataType.SHORT_DATE_TIME)).toTable(db);
      ColumnImpl column=(ColumnImpl)table.getColumn("Value");
      Table calculated=new TableBuilder("CalculatedExactDates").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Value",DataType.SHORT_DATE_TIME).setCalculatedInfo("[ID]")).toTable(db);
      boolean rounded=false;
      for(int i=0;i<bits.length;i++) {
        Object value=column.read(ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(bits[i]).array());
        if(column.write(value,0,ByteOrder.LITTLE_ENDIAN).getLong(0)!=bits[i]) throw new AssertionError("DATE bit loss");
        rounded|=Double.doubleToRawLongBits(column.toDateDouble(value))!=bits[i];
        table.addRow(i,value);
        calculated.addRow(i,value);
      }
      table.addRow(bits.length,null);
      calculated.addRow(bits.length,null);
      if(!rounded) throw new AssertionError("Precision probes did not detect calendar normalization");
      int i=0;
      for(Row row:table) {
        Object value=row.get("Value");
        if(i<bits.length && column.write(value,0,ByteOrder.LITTLE_ENDIAN).getLong(0)!=bits[i])
          throw new AssertionError("Stored DATE bits changed");
        if(i==bits.length && value!=null) throw new AssertionError("NULL changed");
        i++;
      }
      if(i!=bits.length+1) throw new AssertionError("Row count changed");
      i=0;
      for(Row row:calculated) {
        Object value=row.get("Value");
        // Serialize via the ordinary column to verify the cached carrier's
        // exact bits independently of calculated wrapper offsets.
        if(i<bits.length && column.write(value,0,ByteOrder.LITTLE_ENDIAN).getLong(0)!=bits[i])
          throw new AssertionError("Cached DATE bits changed");
        if(i==bits.length && value!=null) throw new AssertionError("Cached NULL changed");
        i++;
      }
      if(i!=bits.length+1) throw new AssertionError("Cached row count changed");
    }
    System.out.println("Exact DATE serialization retains bits that calendar conversion changes.");
  }
}
