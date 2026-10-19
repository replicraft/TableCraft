import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;

/** Original explicit scalar getter oracle; never expands collections or expressions. */
public class ScalarProjectionOracle {
  public static void main(String[] args) throws Exception {
    if(args.length < 3) throw new IllegalArgumentException("Expected database, table and explicit columns");
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table = db.getTable(args[1]);
      for(Row row : table) {
        StringBuilder output = new StringBuilder();
        for(int i = 2; i < args.length; ++i) {
          Column column = table.getColumn(args[i]);
          if(column.getComplexInfo() != null || column.isCalculated()) throw new IllegalArgumentException("Explicit ordinary scalar required");
          if(i != 2) output.append('\t');
          output.append(MultivalueOracle.encode(row.get(column.getName()), column.getType() == DataType.MEMO ? DataType.TEXT : column.getType()));
        }
        System.out.println(output);
      }
    }
  }
}
