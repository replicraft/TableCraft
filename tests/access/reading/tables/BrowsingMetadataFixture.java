import com.healthmarketscience.jackcess.*;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original MIT recipe: local and unresolved linked-table declarations. */
public class BrowsingMetadataFixture {
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
      new TableBuilder("Items")
        .addColumn(new ColumnBuilder("Id", DataType.LONG))
        .addColumn(new ColumnBuilder("Score", DataType.LONG))
        .toTable(db).addRow(1, 20);
      db.createLinkedTable("Unresolved link", "tablecraft-unavailable-backend.mdb", "RemoteItems");
    }
  }
}
