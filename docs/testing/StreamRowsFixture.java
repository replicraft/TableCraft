import com.healthmarketscience.jackcess.*;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert dense table for bounded progressive scalar reads. */
public class StreamRowsFixture {
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
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table table = new TableBuilder("StreamRows")
          .addColumn(new ColumnBuilder("ID", DataType.LONG))
          .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(150))
          .toTable(db);
      for (int id = 1; id <= 20000; id++) {
        table.addRow(id, id % 17 == 0 ? null : "row\t\"" + id + "\n雪🦀" + "x".repeat(96));
      }
      db.flush();
    }
    System.out.println("Original 20,000-row stream input created; no SQL executed.");
  }
}
