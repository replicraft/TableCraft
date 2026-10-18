import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

// Original inert engineering recipe. Never overwrites an existing file.
public class ConstraintFixture {
  interface Attempt { void run() throws Exception; }
  static void rejects(Attempt attempt) throws Exception {
    try { attempt.run(); }
    catch (ConstraintViolationException expected) { return; }
    throw new IllegalStateException("Expected index constraint rejection");
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2000/V2007");
    if (args[0].equals("--inspect")) { inspect(Path.of(args[1])); return; }
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch(args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      for (String name : new String[]{"UniqueValues", "RequiredValues", "PrimaryValues"}) {
        IndexBuilder index = new IndexBuilder("Key").addColumns("Value");
        if (name.equals("UniqueValues")) index.setUnique();
        if (name.equals("RequiredValues")) index.setRequired();
        if (name.equals("PrimaryValues")) index.setPrimaryKey();
        Table table = new TableBuilder(name).addColumn(new ColumnBuilder("Value", DataType.LONG))
          .addIndex(index).toTable(db);
        table.addRow(-1); table.addRow(7);
        if (name.equals("UniqueValues")) {
          table.addRow((Object)null); table.addRow((Object)null);
          rejects(() -> table.addRow(7));
        } else {
          rejects(() -> table.addRow((Object)null));
          if (name.equals("PrimaryValues")) rejects(() -> table.addRow(7));
          else table.addRow(7);
        }
      }
    }
    inspect(path);
  }
  static void inspect(Path path) throws Exception {
    try (Database db = new DatabaseBuilder(path).setReadOnly(true).open()) {
      for (String name : new String[]{"UniqueValues", "RequiredValues", "PrimaryValues"}) {
        Table table = db.getTable(name);
        Index index = table.getIndex("Key");
        System.out.printf("%s\t%s\t%s\t%s\t%s\t%d%n", name, index.isPrimaryKey(),
          index.isUnique(), index.isRequired(), index.shouldIgnoreNulls(), table.getRowCount());
      }
    }
  }
}
