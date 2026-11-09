import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

/** Original inert property/getter oracle; no stored validation expression executes. */
public class ValidationTextCallOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected original database path");
        }
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            List<String> names = new ArrayList<>(database.getTableNames());
            names.sort(String::compareTo);
            for (String name : names) {
                Table table = database.getTable(name);
                System.out.printf("T\t%s\t%s%n", name, scalar(table.getProperties().getValue("ValidationRule")));
                for (Column column : table.getColumns()) {
                    System.out.printf("C\t%s\t%s\t%s\t%s%n", name, column.getName(), column.getType(),
                        scalar(column.getProperties().getValue("ValidationRule")));
                }
                int ordinal = 0;
                for (Row row : table) {
                    System.out.printf("R\t%s\t%d\t%s\t%s\t%s%n", name, ordinal++,
                        scalar(row.get("ID")), scalar(row.get("Label")), scalar(row.get("Other")));
                }
            }
        }
    }

    private static String scalar(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof Integer integer) {
            return "I32:" + integer;
        }
        if (value instanceof String text) {
            return "T" + HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_8));
        }
        throw new IllegalArgumentException("Unexpected original property/getter carrier");
    }
}
