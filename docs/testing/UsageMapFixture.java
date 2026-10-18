import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;

/** Original inert benchmark recipe: one large Binary row per owned data page. */
public class UsageMapFixture {
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
      TableBuilder builder = new TableBuilder("Pages").addColumn(new ColumnBuilder("ID", DataType.LONG));
      for(int col = 0; col < 12; ++col) builder.addColumn(new ColumnBuilder("B" + col, DataType.BINARY).setLength(240));
      Table table = builder.addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey()).toTable(db);
      for(int row = 0; row < 2000; ++row) {
        Object[] values = new Object[13];
        values[0] = row;
        for(int col = 0; col < 12; ++col) {
          byte[] bytes = new byte[240];
          for(int i = 0; i < bytes.length; ++i) bytes[i] = (byte)(row + col + i);
          values[col + 1] = bytes;
        }
        table.addRow(values);
      }
    }
  }
}
