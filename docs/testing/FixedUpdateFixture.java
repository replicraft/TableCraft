import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;
import java.util.*;
import java.time.LocalDateTime;

/** Original inert ACE 16 fixed scalar recipe; no expressions or active content. */
public class FixedUpdateFixture {
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table table=new TableBuilder("Items")
        .addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("Tiny",DataType.BYTE))
        .addColumn(new ColumnBuilder("Narrow",DataType.INT))
        .addColumn(new ColumnBuilder("Number",DataType.LONG))
        .addColumn(new ColumnBuilder("Large",DataType.BIG_INT))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addColumn(new ColumnBuilder("S",DataType.FLOAT))
        .addColumn(new ColumnBuilder("D",DataType.DOUBLE))
        .addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME))
        .addColumn(new ColumnBuilder("G",DataType.GUID))
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO))
        .addColumn(new ColumnBuilder("Payload",DataType.OLE))
        .addIndex(new IndexBuilder("PK_Items").addColumns("Id").setPrimaryKey()).toTable(db);
      for(int id=1;id<=3;id++) {
        byte[] payload=new byte[12288];Arrays.fill(payload,(byte)(id+160));
        table.addRow(id,(byte)1,Short.MIN_VALUE,Integer.MIN_VALUE,Long.MIN_VALUE,
          BigDecimal.valueOf(Long.MIN_VALUE,4),Float.intBitsToFloat(0x80000000),Double.longBitsToDouble(0x8000000000000000L),
          LocalDateTime.of(1899,12,29,6,0),"{00112233-4455-6677-8899-AABBCCDDEEFF}","Unicode é🦀 ".repeat(700),payload);
      }
    }
    System.out.println("Original exact scalar and long-value fixture created; no application content executed.");
  }
}
