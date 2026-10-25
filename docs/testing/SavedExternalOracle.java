import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.query.BaseSelectQuery;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;

/** Getter-only inspection; never asks a query or imported expression to execute. */
public final class SavedExternalOracle {
    private static String encoded(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof byte[] bytes) {
            return "B" + Base64.getEncoder().encodeToString(bytes);
        }
        if (value instanceof Number) {
            return "I" + value;
        }
        return "T" + Base64.getEncoder().encodeToString(
            value.toString().getBytes(StandardCharsets.UTF_8));
    }

    private static void emit(Object... values) {
        StringBuilder line = new StringBuilder();
        for (Object value : values) {
            if (line.length() != 0) {
                line.append('\t');
            }
            line.append(encoded(value));
        }
        System.out.println(line);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected DATABASE");
        }

        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No external target may be resolved");
            });

            for (Query query : database.getQueries()) {
                emit("QUERY", query.getName(), query.getObjectId(), query.getObjectFlag(),
                    query.getType().toString(), query.toSQLString());
                if (query instanceof BaseSelectQuery select) {
                    emit("REMOTE", query.getName(), select.getFromRemoteDbPath(), select.getFromRemoteDbType());
                }
            }

            for (Row row : database.getSystemTable("MSysObjects")) {
                if (Short.valueOf((short) 5).equals(row.get("Type"))) {
                    emit("OBJECT", row.get("Name"), row.get("Id"), row.get("ParentId"), row.get("Flags"));
                }
            }
            for (Row row : database.getSystemTable("MSysQueries")) {
                emit("RAW", row.get("ObjectId"), row.get("Attribute"), row.get("Expression"), row.get("Flag"),
                    row.get("LvExtra"), row.get("Name1"), row.get("Name2"), row.get("Order"));
            }

            for (String name : database.getTableNames()) {
                Table table = database.getTable(name);
                for (Row row : table) {
                    if (name.equals("People")) {
                        emit("CELL", name, row.get("ID"));
                    } else if (name.equals("Orders")) {
                        emit("CELL", name, row.get("ID"), row.get("Person"), row.get("Quarter"));
                    }
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }
}
