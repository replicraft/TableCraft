import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.util.List;
public class CatalogOracle {
  public static void main(String[] args) throws Exception {
    try (Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      Cursor cursor = CursorBuilder.createCursor(db.getSystemTable("MSysObjects"));
      Row row;
      while ((row = cursor.getNextRow(List.of("Id", "ParentId", "Type", "Flags", "Name"))) != null) {
        System.out.printf("%d\t%d\t%d\t%d\t%s%n", row.get("Id"), row.get("ParentId"), row.get("Type"), Integer.toUnsignedLong((Integer)row.get("Flags")), row.get("Name"));
      }
    }
  }
}
