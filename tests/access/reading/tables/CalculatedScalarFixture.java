import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.math.BigDecimal;
import java.time.LocalDateTime;

/** Original inert cached-scalar recipe; expression evaluation stays disabled. */
public class CalculatedScalarFixture {
  public static void main(String[] args) throws Exception {
    if(args.length==3 && args[0].equals("--decode-text")) {
      try(Database db=new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
        db.setEvaluateExpressions(false);
        byte[] payload=java.util.HexFormat.of().parseHex(args[2]);
        java.nio.ByteBuffer wrapped=java.nio.ByteBuffer.allocate(23+payload.length).order(java.nio.ByteOrder.LITTLE_ENDIAN);
        wrapped.putInt(16,payload.length);wrapped.position(20);wrapped.put(payload);
        var column=(com.healthmarketscience.jackcess.impl.ColumnImpl)db.getTable("CalculatedScalars").getColumn("Label");
        String value=(String)column.read(wrapped.array(),java.nio.ByteOrder.LITTLE_ENDIAN);
        System.out.println(java.util.HexFormat.of().formatHex(value.getBytes(java.nio.charset.StandardCharsets.UTF_8)));
      }
      return;
    }
    if(args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table table=builder("CalculatedScalars").toTable(db);
      table.addRow(1,true,-0.0d,new BigDecimal("-1.2345"),LocalDateTime.of(2000,1,2,3,4,5),"ASCII","ASCII");
      table.addRow(2,false,Double.MIN_VALUE,new BigDecimal("0.0000"),LocalDateTime.of(1899,12,29,6,0),"","");
      table.addRow(3,null,null,null,null,null,null);
      table.addRow(4,true,Double.MAX_VALUE,new BigDecimal("922337203685477.5807"),LocalDateTime.of(2024,2,29,12,0),"é🦀\t\"\\\n","é🦀\t\"\\\n");
      table.addRow(5,false,-1.25d,new BigDecimal("-922337203685477.5808"),LocalDateTime.of(1900,1,1,0,0),"x".repeat(40),"x".repeat(40));
      builder("EmptyCalculatedScalars").toTable(db);
    }
  }
  static TableBuilder builder(String name) {
    return new TableBuilder(name)
      .addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN).setCalculatedInfo("[Id]>0"))
      .addColumn(new ColumnBuilder("Number",DataType.DOUBLE).setCalculatedInfo("[Id]/2"))
      .addColumn(new ColumnBuilder("Amount",DataType.MONEY).setCalculatedInfo("[Id]/10000"))
      .addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME).setCalculatedInfo("DateSerial(2000,1,[Id])"))
      .addColumn(new ColumnBuilder("Label",DataType.TEXT).setLengthInUnits(40).setCalculatedInfo("\"prefix\" & [Id]"))
      .addColumn(new ColumnBuilder("Compact",DataType.TEXT).setLengthInUnits(40).setCompressedUnicode(true).setCalculatedInfo("\"prefix\" & [Id]"));
  }
}
