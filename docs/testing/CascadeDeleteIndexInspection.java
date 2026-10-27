import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import java.nio.file.Path;

/** Read-only complete logical index orders for the fresh cascade corpus. */
public final class CascadeDeleteIndexInspection {
    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 1) {
            throw new IllegalArgumentException("Expected database path");
        }

        try (Database database = new DatabaseBuilder(Path.of(arguments[0]))
                .setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked target is authorized");
            });

            for (String name : database.getTableNames()) {
                var table = database.getTable(name);
                for (var index : table.getIndexes()) {
                    System.out.printf("INDEX\t%s\t%s%n", name, index.getName());
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ENTRY\t%s\t%s\t%d%n", name,
                            index.getName(), row.getInt("ID"));
                    }
                }
            }

            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Read-only inspection opened a linked target");
            }
        }
    }
}
