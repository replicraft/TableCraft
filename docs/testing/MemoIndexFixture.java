import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.*;

// Original inert physical Memo indexes; native Access creation is unqualified.
public class MemoIndexFixture {
  static TableBuilder table(String name, int version) {
    ColumnBuilder memo = new ColumnBuilder("Label", DataType.MEMO);
    memo.setTextSortOrder(version == 0 ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
    return new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG)).addColumn(memo);
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 3 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Expected fresh path, format and General version 0/1");
    int version = Integer.parseInt(args[2]);
    if(version != 0 && version != 1) throw new IllegalArgumentException("Unsupported General version");
    String[] values = {null, "", "a", "A  ", "é", "a".repeat(255) + "left", "a".repeat(255) + "right", "a".repeat(254) + "😀" + "tail".repeat(2000), "a".repeat(254) + " " + "Z".repeat(1000), "漢字".repeat(6000), "\u06d6".repeat(255) + "memo", "😀".repeat(127) + "x" + "y".repeat(1024)};
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
      db.setEvaluateExpressions(false);
      Table indexed = table("MemoIndexed", version)
        .addIndex(new IndexBuilder("MemoAsc").addColumns("Label"))
        .addIndex(new IndexBuilder("MemoDesc").addColumns(false, "Label")).toTable(db);
      for(int i = 0; i < 128; i++) indexed.addRow(i, values[(i * 7) % values.length]);
      Table checked = table("MemoChecked", version).addIndex(new IndexBuilder("CheckedMemo").addColumns("Label")).toTable(db);
      for(int i = 0; i < values.length; i++) checked.addRow(i, values[i]);
      table("MemoEmpty", version).addIndex(new IndexBuilder("EmptyMemo").addColumns("Label")).toTable(db);
      table("MemoUnique", version).addIndex(new IndexBuilder("UniqueMemo").addColumns("Label").setUnique()).toTable(db).addRow(1, "small");
    }
  }
}
