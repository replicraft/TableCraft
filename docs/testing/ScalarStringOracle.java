import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;

/** Public raw-cell getters; no reference SQL or imported expression evaluation. */
public final class ScalarStringOracle {
    private static String text(Object value) {
        return value == null ? "N" : "T" + Base64.getEncoder().encodeToString(
            value.toString().getBytes(StandardCharsets.UTF_8));
    }

    public static void main(String[] args) throws Exception {
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("Getter may not open a linked target");
            });

            var table = database.getTable("TextOperands");
            for (var column : table.getColumns()) {
                System.out.println("C\t" + column.getName() + "\t" + column.getType());
            }
            for (Row row : table) {
                System.out.println("R\t" + row.getInt("ID") + "\t" + text(row.getString("Spelling"))
                    + "\t" + text(row.getInt("Number")) + "\t" + text(row.getBoolean("Flag"))
                    + "\t" + text(row.getBigDecimal("Money")) + "\t" + text(row.get("D")));
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }
}
