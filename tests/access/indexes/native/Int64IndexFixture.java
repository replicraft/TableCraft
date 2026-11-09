import com.healthmarketscience.jackcess.*;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert index fixture; generates only a fresh disposable database. */
public class Int64IndexFixture {
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch (args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table table = new TableBuilder("LargeIndexed")
        .addColumn(new ColumnBuilder("ID", DataType.LONG))
        .addColumn(new ColumnBuilder("Value", DataType.BIG_INT))
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("BigAsc").addColumns("Value"))
        .addIndex(new IndexBuilder("BigDesc").addColumns(false, "Value"))
        .toTable(db);
      Long[] keys = {Long.MIN_VALUE, Long.MIN_VALUE + 1, -9007199254740993L, -1L,
        0L, 1L, 9007199254740992L, 9007199254740993L, Long.MAX_VALUE - 1, Long.MAX_VALUE, null};
      for (int i = 0; i < 600; i++) table.addRow(i, keys[(i * 7) % keys.length]);
      Table unique = new TableBuilder("LargeUnique")
        .addColumn(new ColumnBuilder("Value", DataType.BIG_INT))
        .addIndex(new IndexBuilder("BigUnique").addColumns("Value").setUnique())
        .toTable(db);
      for (Long key : keys) unique.addRow(key);
      unique.addRow((Object) null);
      try {
        unique.addRow(Long.MAX_VALUE);
        throw new AssertionError("Duplicate accepted");
      } catch (com.healthmarketscience.jackcess.ConstraintViolationException expected) { }
      new TableBuilder("LargeEmpty")
        .addColumn(new ColumnBuilder("Value", DataType.BIG_INT))
        .addIndex(new IndexBuilder("BigEmpty").addColumns("Value"))
        .toTable(db);
    }
  }
}
