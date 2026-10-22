import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;
import java.util.TimeZone;

/** Original inert 20,000-row index inputs; no SQL or active objects. */
public class LargeIndexFixture {
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and format");
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch (args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    boolean largeNumber = format == Database.FileFormat.V2016 || format == Database.FileFormat.V2019;
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).setAutoSync(false).create()) {
      db.setEvaluateExpressions(false);
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      db.setTimeZone(TimeZone.getTimeZone("UTC"));
      ColumnBuilder label = new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(20);
      label.setTextSortOrder(format == Database.FileFormat.V2000
          ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
      TableBuilder builder = new TableBuilder("IndexedRows")
          .addColumn(new ColumnBuilder("ID", DataType.LONG))
          .addColumn(new ColumnBuilder("Bucket", DataType.LONG))
          .addColumn(new ColumnBuilder("ByteValue", DataType.BYTE))
          .addColumn(new ColumnBuilder("ShortValue", DataType.INT))
          .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN))
          .addColumn(new ColumnBuilder("Money", DataType.MONEY))
          .addColumn(new ColumnBuilder("SingleValue", DataType.FLOAT))
          .addColumn(new ColumnBuilder("DoubleValue", DataType.DOUBLE))
          .addColumn(new ColumnBuilder("Stamp", DataType.SHORT_DATE_TIME))
          .addColumn(new ColumnBuilder("GuidValue", DataType.GUID))
          .addColumn(new ColumnBuilder("BytesValue", DataType.BINARY).setLength(4))
          .addColumn(label)
          .addColumn(new ColumnBuilder("Amount", DataType.NUMERIC).setPrecision(18).setScale(3))
          .addIndex(new IndexBuilder("IdAsc").addColumns("ID").setPrimaryKey())
          .addIndex(new IndexBuilder("IdDesc").addColumns(false, "ID"))
          .addIndex(new IndexBuilder("BucketAsc").addColumns("Bucket"))
          .addIndex(new IndexBuilder("BucketDesc").addColumns(false, "Bucket"))
          .addIndex(new IndexBuilder("Pair").addColumns(false, "Bucket").addColumns("ID"))
          .addIndex(new IndexBuilder("ByteAsc").addColumns("ByteValue"))
          .addIndex(new IndexBuilder("ShortDesc").addColumns(false, "ShortValue"))
          .addIndex(new IndexBuilder("BoolAsc").addColumns("Flag"))
          .addIndex(new IndexBuilder("MoneyAsc").addColumns("Money"))
          .addIndex(new IndexBuilder("SingleAsc").addColumns("SingleValue"))
          .addIndex(new IndexBuilder("DoubleDesc").addColumns(false, "DoubleValue"))
          .addIndex(new IndexBuilder("StampAsc").addColumns("Stamp"))
          .addIndex(new IndexBuilder("GuidAsc").addColumns("GuidValue"))
          .addIndex(new IndexBuilder("GuidDesc").addColumns(false, "GuidValue"))
          .addIndex(new IndexBuilder("BinaryAsc").addColumns("BytesValue"))
          .addIndex(new IndexBuilder("TextAsc").addColumns("Label"))
          .addIndex(new IndexBuilder("AmountDesc").addColumns(false, "Amount"));
      if (largeNumber) {
        builder.addColumn(new ColumnBuilder("BigValue", DataType.BIG_INT))
            .addIndex(new IndexBuilder("BigAsc").addColumns("BigValue"));
      }
      Table table = builder.toTable(db);
      String[] guids = {"{00000000-0000-0000-0000-000000000000}",
          "{6B29FC40-CA47-1067-B31D-00DD010662DA}", "{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}"};
      for (int id = 1; id <= 20000; id++) {
        int group = id % 3;
        boolean missing = id % 5 == 0;
        List<Object> row = new ArrayList<>();
        row.add(id);
        row.add(missing ? null : group - 1);
        row.add(missing ? null : new int[]{0, 127, 255}[group]);
        row.add(missing ? null : new short[]{Short.MIN_VALUE, 0, Short.MAX_VALUE}[group]);
        row.add(id % 2 == 0);
        row.add(missing ? null : BigDecimal.valueOf(group - 1, 4));
        row.add(missing ? null : new float[]{-0.0f, Float.MIN_VALUE, 30.5f}[group]);
        row.add(missing ? null : new double[]{-0.0, Double.MIN_VALUE, 1.0e200}[group]);
        row.add(missing ? null : LocalDateTime.of(2024, 1, 1 + group, 12, 0));
        row.add(missing ? null : guids[group]);
        row.add(missing ? null : new byte[]{(byte) group, 0, (byte) 255, 1});
        row.add(missing ? null : new String[]{"a", "b", "é"}[group]);
        row.add(missing ? null : BigDecimal.valueOf(group - 1, 3));
        if (largeNumber) row.add(missing ? null : 9007199254740993L + group);
        table.addRow(row.toArray());
      }
      db.flush();
    }
    System.out.println("Original 20,000-row indexed input created; no SQL executed.");
  }
}
