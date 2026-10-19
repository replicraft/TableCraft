import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;

/** Original explicit scalar getter oracle; never expands collections or expressions. */
public class ScalarProjectionOracle {
  public static void main(String[] args) throws Exception {
    if(args.length < 3) throw new IllegalArgumentException("Expected database, table and explicit columns");
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table = db.getTable(args[1]);
      boolean indexed = args[2].equals("--index");
      int first = indexed ? 4 : 2;
      if(first >= args.length) throw new IllegalArgumentException("Expected explicit scalar columns");
      Cursor cursor = indexed ? CursorBuilder.createCursor(table.getIndex(args[3])) : CursorBuilder.createCursor(table);
      Row row;
      while((row = cursor.getNextRow()) != null) {
        StringBuilder output = new StringBuilder();
        for(int i = first; i < args.length; ++i) {
          Column column = table.getColumn(args[i]);
          if(column.getComplexInfo() != null || column.isCalculated()) throw new IllegalArgumentException("Explicit ordinary scalar required");
          if(i != first) output.append('\t');
          output.append(MultivalueOracle.encode(row.get(column.getName()), column.getType() == DataType.MEMO ? DataType.TEXT : column.getType()));
        }
        System.out.println(output);
      }
    }
  }
}
