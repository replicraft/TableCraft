import com.healthmarketscience.jackcess.*;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert stored-result fixture. Expressions are deliberately disabled. */
public class CalculatedStorageFixture {
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
      db.setEvaluateExpressions(false);
      Table table = new TableBuilder("Calculations")
        .addColumn(new ColumnBuilder("Score", DataType.LONG))
        .addColumn(new ColumnBuilder("Twice", DataType.LONG).setCalculatedInfo("[Score]*2"))
        .toTable(db);
      table.addRow(10,20); table.addRow(-20,-40); table.addRow(0,0);
      table.addRow(null,null);
      table.addRow(3,41); // Deliberately stale stored result proves no recomputation.
      table.addRow(0,Integer.MIN_VALUE); table.addRow(0,Integer.MAX_VALUE);
      new TableBuilder("EmptyCalculations")
        .addColumn(new ColumnBuilder("Score", DataType.LONG))
        .addColumn(new ColumnBuilder("Twice", DataType.LONG).setCalculatedInfo("[Score]*2"))
        .toTable(db);
    }
  }
}
