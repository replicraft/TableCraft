import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;

/** Original ordinary-table producer and inert reopened public-getter oracle. */
public final class ProgressiveQueryFixture {
    private static final int ROWS = 20_000;

    private static String label(int id) {
        if (id % 7 == 0) {
            return null;
        }
        return switch (id % 5) {
            case 0 -> "row\t" + id;
            case 1 -> "é🦀" + id;
            case 2 -> "quote\"" + id;
            case 3 -> "line\n" + id;
            default -> "nul\0" + id;
        };
    }

    private static void write(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            database.setEvaluateExpressions(false);
            Table table = new TableBuilder("StreamingRows")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(64))
                .toTable(database);
            for (int id = 1; id <= ROWS; id++) {
                table.addRow(id, id % 11 == 0 ? null : id - 10_000, label(id));
            }
        }
    }

    private static void read(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("Linked opens are forbidden");
            });
            Table table = database.getTable("StreamingRows");
            for (Column column : table.getColumns()) {
                System.out.println("COLUMN\t" + column.getName() + "\t" + column.getType());
            }
            for (Row row : table) {
                Integer score = row.getInt("Score");
                String text = row.getString("Label");
                String encoded = text == null ? "N" : "T" + Base64.getEncoder()
                    .encodeToString(text.getBytes(StandardCharsets.UTF_8));
                System.out.println("ROW\t" + row.getInt("ID") + "\t"
                    + (score == null ? "N" : score) + "\t" + encoded);
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("write")) {
            write(Path.of(args[1]), args[2]);
        } else if (args.length == 2 && args[0].equals("read")) {
            read(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected write PATH FORMAT or read PATH");
        }
    }
}
