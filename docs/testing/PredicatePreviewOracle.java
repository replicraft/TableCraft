import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original read-only predicate declaration/input/cache getter oracle. */
public class PredicatePreviewOracle {
    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 1) {
            throw new IllegalArgumentException("Expected original path");
        }
        try (Database database = new DatabaseBuilder(Path.of(arguments[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            for (String name : new String[] {"Predicates", "EmptyPredicates"}) {
                Table table = database.getTable(name);
                for (Column field : table.getColumns()) {
                    if (!field.isCalculated()) {
                        if (field.getType() != DataType.LONG) {
                            throw new AssertionError("Unexpected ordinary input declaration");
                        }
                        continue;
                    }
                    Object result = field.getProperties().getValue("ResultType");
                    Object expression = field.getProperties().getValue("Expression");
                    if (!(result instanceof Byte) || !(expression instanceof String)) {
                        throw new AssertionError("Unexpected typed calculation properties");
                    }
                    System.out.printf("D\t%s\t%s\t%s\t%d\t%d\t%s%n", name, field.getName(),
                        field.getType(), field.getLength(), Byte.toUnsignedInt((Byte) result),
                        HexFormat.of().formatHex(((String) expression).getBytes(StandardCharsets.UTF_8)));
                }
                for (Row row : table) {
                    StringBuilder output = new StringBuilder("R\t").append(name);
                    output.append('\t').append(row.get("ID"));
                    output.append('\t').append(row.get("Input") == null ? "N" : row.get("Input"));
                    for (Column field : table.getColumns()) {
                        if (field.isCalculated()) {
                            Object value = row.get(field.getName());
                            output.append('\t').append(value == null ? "N" : ((Boolean) value ? "T" : "F"));
                        }
                    }
                    System.out.println(output);
                }
            }
        }
    }
}
