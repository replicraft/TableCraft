import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.complex.VersionHistoryColumnInfoImpl;
import java.nio.*;
import java.nio.file.*;
import java.util.*;

/** Original inert stored-history recipe; exact DATE carriers and visible Memo binding. */
// DateExt plus the Date overload deliberately preserve serial bits that a civil
// LocalDateTime round trip cannot retain, including signed zero/subnormals.
@SuppressWarnings("deprecation")
public class VersionHistoryFixture {
  static java.util.Date date(ColumnImpl column,long bits)throws Exception {
    return (java.util.Date)column.read(ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(bits).array());
  }
  static long bits(ColumnImpl column,Object value)throws Exception {
    return column.write(value,0,ByteOrder.LITTLE_ENDIAN).duplicate().order(ByteOrder.LITTLE_ENDIAN).getLong();
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.DATE);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table table=new TableBuilder("Histories").addColumn(new ColumnBuilder("ID",DataType.LONG).setAutoNumber(true))
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO).setAppendOnly(true))
        .addColumn(new ColumnBuilder("Details",DataType.MEMO))
        .addColumn(new ColumnBuilder("Plain",DataType.MEMO)).toTable(db);
      Column notes=table.getColumn("Notes").getVersionHistoryColumn();
      var info=(VersionHistoryColumnInfoImpl)notes.getComplexInfo();
      ColumnImpl modified=(ColumnImpl)info.getModifiedDateColumn();
      long[] exact={0,0x8000000000000000L,1,0x8000000000000001L,
        Double.doubleToRawLongBits(0.25),Double.doubleToRawLongBits(-0.25),
        Double.doubleToRawLongBits(-1.25),Double.doubleToRawLongBits(-1.75),
        Double.doubleToRawLongBits(1.0000000001),Double.doubleToRawLongBits(-1.0000000001),
        Double.doubleToRawLongBits(45000.123456789),Double.doubleToRawLongBits(-657434.25),
        Double.doubleToRawLongBits(2958465.9999999995)};
      for(int ordinal=0;ordinal<4;ordinal++) {
        Object[] row=table.addRow(Column.AUTO_NUMBER,null,null,null);
        var key=(ComplexValueForeignKey)notes.getRowValue(row);
        if(ordinal==0) {
          for(int i=0;i<exact.length;i++)key.addVersion(i==0?"":i==1?"é🦀\0line\r\n":"same",date(modified,exact[i]));
          List<Row> stored=info.getRawValues(key.get());
          if(stored.size()!=exact.length)throw new AssertionError("Version count changed");
          for(int i=0;i<exact.length;i++)if(bits(modified,stored.get(i).get(modified.getName()))!=exact[i])
            throw new AssertionError("Stored version DATE bits changed");
        }
        if(ordinal==1) {
          key.addVersion("first-at-same-time",date(modified,Double.doubleToRawLongBits(45000.25)));
          key.addVersion("second-at-same-time",date(modified,Double.doubleToRawLongBits(45000.25)));
          if(!key.getVersions().get(0).getValue().equals("second-at-same-time"))
            throw new AssertionError("Reference history ordering control changed");
        }
        if(ordinal==3)key.addVersion(null,(java.util.Date)null);
      }
      // This reviewed builder permits one append-only Memo per table. A second
      // table tests a different visible name and independently scoped parent keys.
      Table other=new TableBuilder("OtherHistories").addColumn(new ColumnBuilder("ID",DataType.LONG).setAutoNumber(true))
        .addColumn(new ColumnBuilder("Details",DataType.MEMO).setAppendOnly(true)).toTable(db);
      Column hidden=other.getColumn("Details").getVersionHistoryColumn();
      for(int ordinal=0;ordinal<2;ordinal++) {
        Object[] row=other.addRow(Column.AUTO_NUMBER,null);
        var key=(ComplexValueForeignKey)hidden.getRowValue(row);
        key.addVersion(ordinal==0?"separate":null,date(modified,Double.doubleToRawLongBits(0.25)));
      }
    }
    System.out.println("Original exact stored-history assertions passed.");
  }
}
