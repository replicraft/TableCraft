import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

// Original inert scale/history recipe. No Access content or expressions execute.
public class ScaleHistoryFixture {
    private static final int INITIAL_ROWS = 100_000;
    private static final int INSERTION_STRIDE = 7_919;
    private static final int GROW_INTERVAL = 97;
    private static final int DELETE_INTERVAL = 101;
    private static final int APPENDED_ROWS = 100;

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh output path and format");
        }
        Path output = Path.of(args[0]);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2000" -> Database.FileFormat.V2000;
            case "V2007" -> Database.FileFormat.V2007;
            case "V2016" -> Database.FileFormat.V2016;
            default -> throw new IllegalArgumentException("Unsupported engineering format");
        };

        try (Database database = new DatabaseBuilder(output).setFileFormat(format).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            Table table = new TableBuilder("ScaleRows")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
                .toTable(database);

            for (int position = 0; position < INITIAL_ROWS; position++) {
                int id = (int) ((long) position * INSERTION_STRIDE % INITIAL_ROWS) + 1;
                if (position == INITIAL_ROWS / 2) {
                    new ColumnBuilder("Later", DataType.LONG).addToTable(table);
                }
                if (position < INITIAL_ROWS / 2) {
                    table.addRow(id, id % 97 - 48, "n:" + id);
                } else {
                    table.addRow(id, id % 97 - 48, "n:" + id, id * 2);
                }
            }

            Cursor cursor = CursorBuilder.createCursor(table);
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                int id = row.getInt("ID");
                if (id % DELETE_INTERVAL == 0) {
                    cursor.deleteCurrentRow();
                } else if (id % GROW_INTERVAL == 0) {
                    row.put("Notes", "updated:" + id + ":" + "é😀".repeat(128));
                    cursor.updateCurrentRowFromMap(row);
                }
            }
            for (int id = INITIAL_ROWS + 1; id <= INITIAL_ROWS + APPENDED_ROWS; id++) {
                table.addRow(id, id % 97 - 48, "append:" + id, id * 2);
            }
            int expected = INITIAL_ROWS - INITIAL_ROWS / DELETE_INTERVAL + APPENDED_ROWS;
            if (table.getRowCount() != expected) {
                throw new IllegalStateException("Unexpected retained row count");
            }
            System.out.println("Rows\t" + expected);
        }
    }
}
