import com.healthmarketscience.jackcess.*;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original MIT recipe for inert native text reads and LIKE query comparisons. */
public class LikeFixture {
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
      Table table = new TableBuilder("Text samples")
        .addColumn(new ColumnBuilder("ID", DataType.LONG))
        .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(120))
        .toTable(db);
      String[] labels = {null, "", "apple", "Apple", "ab", "b2", "cab", "x", "z", "a1b", "123", "a*b", "*", "?", "#", "[]", "[", "]", "!", "-", "é", "é🙂", "🙂", "\n", "%_", "%", "a ", "NULL"};
      for(int i = 0; i < labels.length; i++) table.addRow(i + 1, labels[i]);
    }
  }
}
