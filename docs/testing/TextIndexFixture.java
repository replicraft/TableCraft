import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.*;

// Original inert fixture inputs. Access/DAO did not author these databases.
public class TextIndexFixture {
  static TableBuilder table(String name, int version) {
    ColumnBuilder text = new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(255);
    text.setTextSortOrder(version == 0 ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
    return new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG)).addColumn(text);
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 3 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Expected fresh path, format and General version 0/1");
    int version = Integer.parseInt(args[2]);
    if(version != 0 && version != 1) throw new IllegalArgumentException("Unsupported General version");
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
      db.setEvaluateExpressions(false);
      Table indexed = table("TextIndexed", version)
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("TextAsc").addColumns("Label"))
        .addIndex(new IndexBuilder("TextDesc").addColumns(false, "Label")).toTable(db);
      String[] values = {null, "", "a", "A", "a  ", "é", "e\u0301", "Straße", "Strasse", "漢字々", "カタカナー", "😀", "a-b", "a'b", "\u0001", "\u0000", "\u0301", "\u200b", "a".repeat(255), "\u06d6".repeat(255), "😀".repeat(127) + "x"};
      for(int i = 0; i < 1000; i++) indexed.addRow(i, values[(i * 17) % values.length]);
      Table checked = table("TextChecked", version).addIndex(new IndexBuilder("CheckedText").addColumns("Label")).toTable(db);
      for(int i = 0; i < 64; i++) checked.addRow(i, values[(i * 17) % values.length]);
      table("TextDigestUnique", version).addIndex(new IndexBuilder("DigestUnique").addColumns("Label").setUnique()).toTable(db).addRow(1, "a".repeat(255));
      Table unique = table("TextUnique", version).addIndex(new IndexBuilder("UniqueText").addColumns("Label").setUnique()).toTable(db);
      for(int i = 0; i < 32; i++) unique.addRow(i, "unique" + i);
      unique.addRow(32, (Object)null);
      table("TextEmpty", version).addIndex(new IndexBuilder("EmptyText").addColumns("Label")).toTable(db);
      table("TextIgnored", version).addIndex(new IndexBuilder("IgnoredText").addColumns("Label").setIgnoreNulls()).toTable(db).addRow(1, "a");
      table("TextCompound", version).addIndex(new IndexBuilder("CompoundText").addColumns("Label", "ID")).toTable(db).addRow(1, "a");
    }
  }
}
