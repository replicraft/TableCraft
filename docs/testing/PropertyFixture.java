import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

// Original inert metadata recipe. Outputs must be fresh; no active code is added.
public class PropertyFixture {
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2000/V2007, or --inspect path");
    if (args[0].equals("--inspect")) { inspect(Path.of(args[1])); return; }
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch (args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      new TableBuilder("Rules")
        .putProperty("ValidationRule", "[Amount] >= 0")
        .putProperty("ValidationText", "Amount must be nonnegative")
        .addColumn(new ColumnBuilder("Amount", DataType.LONG)
          .putProperty("Required", true)
          .putProperty("DefaultValue", "7")
          .putProperty("ValidationRule", ">= 0")
          .putProperty("ValidationText", "No negative amounts")
          .putProperty("FutureFlag", DataType.BINARY, new byte[]{1, 2, 3}))
        .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(40)
          .putProperty("Required", false)
          .putProperty("AllowZeroLength", false)
          .putProperty("DefaultValue", "\"hello\"")
          .putProperty("Caption", "é🦀"))
        .toTable(db);
    }
    inspect(path);
  }
  static void print(String scope, String name, PropertyMap map) {
    for (PropertyMap.Property property : map) {
      Object value = property.getValue();
      String output;
      if (value instanceof Boolean flag) output = flag ? "TRUE" : "FALSE";
      else if (value instanceof String text) output = text;
      else output = "OPAQUE";
      System.out.printf("%s\t%s\t%s\t%d\t0x%02x\t%s%n", scope, name,
        property.getName(), Byte.toUnsignedInt(property.getType().getValue()), property.isDdl() ? 1 : 0, output);
    }
  }
  static void inspect(Path path) throws Exception {
    try (Database db = new DatabaseBuilder(path).setReadOnly(true).open()) {
      Table table = db.getTable("Rules");
      print("Table", "", table.getProperties());
      for (Column column : table.getColumns()) print("Column", column.getName(), column.getProperties());
    }
  }
}
