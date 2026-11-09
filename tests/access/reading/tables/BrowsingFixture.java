import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original MIT recipe: larger tables for bounded sequential browsing. */
public class BrowsingFixture {
  private static final int ROWS = 20_050;
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2000/V2016/V2019");
    Path path = Path.of(args[0]);
    if(Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch(args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported family");
    };
    try(Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table table = new TableBuilder("Browse rows")
        .addColumn(new ColumnBuilder("Record ID", DataType.LONG))
        .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(80))
        .addColumn(new ColumnBuilder("Amount", DataType.NUMERIC).setPrecision(28).setScale(4))
        .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
        .toTable(db);
      for(int i = 1; i <= ROWS; i++) {
        String label = i % 7 == 0 ? null : "row " + i + " é中🦀\t\n\"\\";
        BigDecimal amount = i % 11 == 0 ? null : i % 13 == 0 ? new BigDecimal("-999999999999999999999999.9999") : BigDecimal.valueOf(i * 1234567L, 4);
        String notes = i % 5000 == 0 ? "long " + "中".repeat(5000) : i % 2 == 0 ? "" : null;
        table.addRow(i, label, amount, notes);
      }
    }
  }
}
