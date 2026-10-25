import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Original read-only getter oracle. Never evaluates a stored expression. */
public class CalculatedCarrierOracle {
    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 1) {
            throw new IllegalArgumentException("Expected original path");
        }
        try (Database database = new DatabaseBuilder(Path.of(arguments[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            List<String> names = new ArrayList<>(database.getTableNames());
            names.sort(String::compareTo);
            for (String name : names) {
                Table table = database.getTable(name);
                Column field = table.getColumn("Cached");
                Object result = field.getProperties().getValue("ResultType");
                Object expression = field.getProperties().getValue("Expression");
                if (!(result instanceof Byte) || !"[ID]*2".equals(expression)) {
                    throw new AssertionError("Unexpected original typed calculation properties");
                }
                System.out.printf("D\t%s\t%s\t%d\t%s\t%d%n", name, field.getType(),
                    Short.toUnsignedInt(field.getLength()), field.isCalculated(), Byte.toUnsignedInt((Byte) result));
                int ordinal = 0;
                for (Row row : table) {
                    System.out.printf("R\t%s\t%d\t%d\t%s%n", name, ordinal++, row.get("ID"),
                        scalar(field.getType(), row.get("Cached")));
                }
            }
        }
    }

    private static String scalar(DataType type, Object value) {
        if (value == null) {
            return "N";
        }
        return switch (type) {
            case BYTE -> "U" + (((Number) value).intValue() & 0xff);
            case INT, BIG_INT -> "I:" + value;
            case FLOAT -> String.format("F%08x", Float.floatToRawIntBits(((Number) value).floatValue()));
            case GUID -> "G" + value.toString().replace("{", "").replace("}", "");
            default -> throw new AssertionError("Unexpected carrier");
        };
    }
}
