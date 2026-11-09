import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.nio.file.Path;

/** Original inert mixed-page oracle, using the existing exact scalar encoder. */
public class MultivaluePageOracle {
  public static void main(String[] args) throws Exception {
    if(args.length != 2) throw new IllegalArgumentException("Expected path and table");
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table = db.getTable(args[1]);
      for(Column column : table.getColumns()) {
        String kind = column.getComplexInfo() != null ? "M" : column.isCalculated() ? "C" : "S";
        System.out.println("H\t" + column.getName() + "\t" + kind);
      }
      int ordinal = 0;
      for(Row row : table) {
        for(Column column : table.getColumns()) {
          String prefix = ordinal + "\t" + column.getName();
          Object value = row.get(column.getName());
          if(column.getComplexInfo() != null) {
            if(column.getComplexInfo().getType() != ComplexDataType.MULTI_VALUE)
              throw new IllegalArgumentException("Expected multivalue template");
            DataType type = ((com.healthmarketscience.jackcess.impl.complex.MultiValueColumnInfoImpl)column.getComplexInfo()).getValueColumn().getType();
            ComplexValueForeignKey key = (ComplexValueForeignKey)value;
            StringBuilder line = new StringBuilder(prefix).append("\tM\t").append(key == null ? "N" : key.get());
            if(key != null) for(SingleValue child : key.getMultiValues())
              line.append('\t').append(MultivalueOracle.encode(child.get(), type));
            System.out.println(line);
          } else {
            System.out.println(prefix + "\tS\t" + MultivalueOracle.encode(value, column.getType()));
          }
        }
        ordinal++;
      }
    }
  }
}
