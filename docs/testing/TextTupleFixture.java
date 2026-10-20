import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.*;
import java.math.BigDecimal;
import java.time.LocalDateTime;

// Original inert physical tuples, not Microsoft Access-generated evidence.
public class TextTupleFixture {
  static ColumnBuilder text(String name, int version, boolean memo) {
    ColumnBuilder column = new ColumnBuilder(name, memo ? DataType.MEMO : DataType.TEXT).setLengthInUnits(memo ? 0 : 255);
    column.setTextSortOrder(version == 0 ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
    return column;
  }
  static TableBuilder pair(String name, int version, boolean memo) {
    return new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG))
      .addColumn(text("Label", version, memo));
  }
  static IndexBuilder index(String name, boolean ascending, String... columns) {
    return new IndexBuilder(name).addColumns(ascending, columns);
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 3 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("fresh path, format, General version");
    int version = Integer.parseInt(args[2]);
    if(version != 0 && version != 1) throw new IllegalArgumentException("General version");
    String[] texts = {null, "", "a", "A  ", "é", "e\u0301", "Straße", "Strasse", "漢字", "カタカナー", "😀", "a-b", "\u0000", "\u06d6".repeat(255), "a".repeat(255), "😀".repeat(127) + "x"};
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
      db.setEvaluateExpressions(false);
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table pair = pair("Pairs", version, false)
        .addIndex(index("TextNumber", true, "Label", "ID"))
        .addIndex(index("NumberText", false, "ID", "Label"))
        .addIndex(new IndexBuilder("MixedDirection").addColumns(true, "Label").addColumns(false, "ID")).toTable(db);
      for(int i=0; i<1000; i++) pair.addRow(i, texts[(i * 7) % texts.length]);
      TableBuilder mixed = new TableBuilder("Mixed")
        .addColumn(new ColumnBuilder("ID", DataType.LONG))
        .addColumn(text("Label", version, false)).addColumn(text("Other", 1-version, false))
        .addColumn(new ColumnBuilder("Number", DataType.INT))
        .addColumn(new ColumnBuilder("Token", DataType.GUID))
        .addColumn(new ColumnBuilder("Amount", DataType.NUMERIC).setPrecision(28).setScale(4))
        .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(255))
        .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("Real", DataType.DOUBLE))
        .addColumn(new ColumnBuilder("Day", DataType.SHORT_DATE_TIME))
        .addColumn(new ColumnBuilder("Price", DataType.MONEY));
      String[] keys = {"Label", "Other", "Number", "Token", "Amount", "Bytes", "Flag", "Real", "Day", "Price"};
      mixed.addIndex(index("TenAsc", true, keys)).addIndex(index("TenDesc", false, keys));
      IndexBuilder alternating = new IndexBuilder("Alternating");
      for(int i=0; i<keys.length; i++) alternating.addColumns(i%2==0, keys[i]);
      Table mt = mixed.addIndex(alternating).toTable(db);
      for(int i=0; i<64; i++) {
        boolean nil = i%13==0;
        byte[] bytes = i%7==0 ? new byte[0] : new byte[(i*37)%255+1];
        for(int j=0;j<bytes.length;j++) bytes[j]=(byte)(i+j);
        mt.addRow(i, texts[i%texts.length], texts[(i*3+1)%texts.length], nil?null:i-32,
          nil?null:String.format("{%08x-1234-5678-abcd-0123456789ab}", i),
          nil?null:new BigDecimal(i-32).movePointLeft(4).setScale(4), nil?null:bytes, i%2==0,
          nil?null:(i-32)/4.0, nil?null:LocalDateTime.of(2020,1,1,0,0).plusDays(i),
          nil?null:new BigDecimal(i-32).movePointLeft(4).setScale(4));
      }
      Table memo = pair("MemoPairs", version, true)
        .addIndex(index("MemoTextNumber", true, "Label", "ID"))
        .addIndex(index("MemoDescending", false, "Label", "ID")).toTable(db);
      for(int i=0;i<16;i++) memo.addRow(i, i%4==0 ? null : texts[i%texts.length] + "tail".repeat(300));
      Table unique = pair("UniquePairs", version, false).addIndex(index("UniquePair", true, "Label", "ID").setUnique()).toTable(db);
      unique.addRow(null, null); unique.addRow(null, null); unique.addRow(1, "a"); unique.addRow(2, "A"); unique.addRow(null, "b");
      pair("MemoUniquePair", version, true).addIndex(index("UniqueMemoPair", true, "Label", "ID").setUnique()).toTable(db).addRow(1,"small");
      pair("EmptyPairs", version, false).addIndex(index("EmptyPair", false, "Label", "ID")).toTable(db);
    }
  }
}
