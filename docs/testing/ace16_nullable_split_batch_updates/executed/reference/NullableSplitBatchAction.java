import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Objects;
import java.util.Set;
import java.util.TimeZone;
import java.util.TreeMap;

/** Closed ordered public UPDATE observations on fresh original-file copies. */
public final class NullableSplitBatchAction {
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final int ROWS = 500;

    private static void bounded(Path path) throws Exception {
        long length = Files.size(path);
        if (length <= 0 || length > MAX_FILE_BYTES || length % 4096 != 0) {
            throw new AssertionError("Bounded complete page image required");
        }
    }

    private static int signed(int ordinal) {
        return (ordinal % 2 == 0)
            ? Integer.MIN_VALUE + ordinal * 17
            : Integer.MAX_VALUE - ordinal * 17;
    }

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setAllowAutoNumberInsert(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
    }

    private static TreeMap<Integer, Row> originalRows(Table table) throws Exception {
        if (table.getRowCount() != ROWS || table.getColumnCount() != 3) {
            throw new AssertionError("Exact original table required");
        }
        TreeMap<Integer, Row> rows = new TreeMap<>();
        for (Row row : table) {
            Integer identity = row.getInt("ID");
            if (identity == null || identity < 1 || identity > ROWS
                    || rows.put(identity, row) != null) {
                throw new AssertionError("Exact unique manual original IDs required");
            }
            int ordinal = identity - 1;
            Integer group = ordinal < 300 ? null : signed(ordinal);
            if (!Objects.equals(row.get("Group"), group) || row.get("Score") != null) {
                throw new AssertionError("Exact original field values required");
            }
        }
        if (rows.size() != ROWS) {
            throw new AssertionError("Complete original population required");
        }
        return rows;
    }

    private static void action(Path source, Path output, String profile,
            String direction, String tableName, String recipe) throws Exception {
        if (!Set.of("V2016", "V2019").contains(profile)
                || !Set.of("ASC", "DESC").contains(direction)
                || !Set.of("Items", "UniqueItems").contains(tableName)
                || !Set.of("Full", "Clear", "DuplicateFamily").contains(recipe)
                || (recipe.equals("DuplicateFamily") && !tableName.equals("Items"))) {
            throw new IllegalArgumentException("Closed original/reference recipe required");
        }
        bounded(source);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Files.copy(source, output);
        int updates = 0;
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel)
                .setAutoSync(false).open()) {
            inert(database);
            Table table = database.getTable(tableName);
            TreeMap<Integer, Row> rows = originalRows(table);
            for (var entry : rows.entrySet()) {
                int ordinal = entry.getKey() - 1;
                Integer group;
                Integer score;
                if (recipe.equals("DuplicateFamily")) {
                    if (ordinal >= 2) {
                        break;
                    }
                    group = 42;
                    score = 777;
                } else if (recipe.equals("Clear") && ordinal >= 300) {
                    group = null;
                    score = null;
                } else {
                    group = signed(ordinal);
                    score = signed(ordinal + ROWS);
                }
                Row row = entry.getValue();
                row.put("Group", group);
                row.put("Score", score);
                table.updateRow(row);
                updates++;
            }
            if (updates != (recipe.equals("DuplicateFamily") ? 2 : ROWS)) {
                throw new AssertionError("Exact ordered public update count required");
            }
            database.flush();
        }
        bounded(output);
        System.out.printf("ACTION\t%s-%s-%s-%s\tsuccess\t%d%n",
            profile, direction, tableName, recipe, updates);
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 7 || !arguments[0].equals("action")) {
            throw new IllegalArgumentException("Only a closed action command is authorized");
        }
        action(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3],
            arguments[4], arguments[5], arguments[6]);
    }
}
