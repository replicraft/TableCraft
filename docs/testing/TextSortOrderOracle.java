import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.DatabaseImpl;
import java.nio.file.*;

// Original inert metadata oracle. Does not evaluate queries or alter the source.
public class TextSortOrderOracle {
  static String raw(ColumnImpl.SortOrder order) {
    return order.getValue() + ":" + order.getVariant() + ":" + order.getVersion();
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 3) throw new IllegalArgumentException("Expected path, table and column");
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Column column = db.getTable(args[1]).getColumn(args[2]);
      System.out.println(raw(((DatabaseImpl)db).getDefaultSortOrder()) + "\t" +
                         raw(((ColumnImpl)column).getTextSortOrder()));
    }
  }
}
