import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.math.BigDecimal;
import java.time.LocalDateTime;
public class ScalarFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2) throw new IllegalArgumentException("Expected output path and V2000/V2007/V2016/V2019");
    Path output=Path.of(args[0]);
    if(java.nio.file.Files.exists(output)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Expected V2000/V2007/V2016/V2019");
    };
    try(Database db=new DatabaseBuilder(output).setFileFormat(format).create()) {
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table table=new TableBuilder("ScalarSamples")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("Octet",DataType.BYTE))
        .addColumn(new ColumnBuilder("Small",DataType.INT))
        .addColumn(new ColumnBuilder("Money",DataType.MONEY))
        .addColumn(new ColumnBuilder("Real",DataType.FLOAT))
        .addColumn(new ColumnBuilder("WideReal",DataType.DOUBLE))
        .addColumn(new ColumnBuilder("Label",DataType.TEXT).setLengthInUnits(20))
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO))
        .addColumn(new ColumnBuilder("Bytes",DataType.BINARY).setLength(16))
        .addColumn(new ColumnBuilder("Opaque",DataType.OLE))
        .addColumn(new ColumnBuilder("G",DataType.GUID))
        .addColumn(new ColumnBuilder("Amount",DataType.NUMERIC).setPrecision(28).setScale(4))
        .addColumn(new ColumnBuilder("Stamp",DataType.SHORT_DATE_TIME))
        .toTable(db);
      byte[] longBytes=new byte[12000];for(int i=0;i<longBytes.length;i++) longBytes[i]=(byte)(i%251);
      table.addRow(1,true,(byte)255,Short.MIN_VALUE,new BigDecimal("-922337203685477.5808"),-0.0f,1.25d,"é中🦀","Long text "+"中".repeat(3000),new byte[]{0,(byte)255},longBytes,"{6b29fc40-ca47-1067-b31d-00dd010662da}",new BigDecimal("999999999999999999999999.9999"),LocalDateTime.of(2025,1,1,12,0));
      table.addRow(2,false,(byte)0,Short.MAX_VALUE,new BigDecimal("0.0000"),1.5f,-0.0d,"","",new byte[0],new byte[0],"{00000000-0000-0000-0000-000000000000}",new BigDecimal("-9007199254740993.1200"),LocalDateTime.of(1899,12,29,6,0));
      table.addRow(3,false,null,null,null,null,null,null,null,null,null,null,null,null);
    }
  }
}
