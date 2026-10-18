import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;

/** Original inert recipe: wide metadata crosses several definition pages. */
public class DefinitionFixture {
  private static final int COLUMNS = 200;
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2000/V2007");
    Path path = Path.of(args[0]);
    if(java.nio.file.Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch(args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try(Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      for(String name : new String[]{"Wide", "EmptyWide"}) {
        TableBuilder builder = new TableBuilder(name);
        for(int i = 0; i < COLUMNS; ++i) builder.addColumn(new ColumnBuilder(column(i), DataType.LONG));
        Table table = builder
          .addIndex(new IndexBuilder("First").addColumns(column(0)).setPrimaryKey())
          .addIndex(new IndexBuilder("LastDesc").addColumns(false, column(COLUMNS - 1)))
          .toTable(db);
        if(name.equals("Wide")) for(int row = 0; row < 7; ++row) {
          Object[] values = new Object[COLUMNS];
          for(int col = 0; col < COLUMNS; ++col) values[col] = row * 1000 + col;
          values[COLUMNS - 1] = row % 3 == 0 ? null : row % 3 - 1;
          table.addRow(values);
        }
      }
    }
  }
  private static String column(int ordinal) {
    return String.format(java.util.Locale.ROOT, "F%03d_", ordinal) + "x".repeat(48) + "é🦀";
  }
}
