import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;

// Original inert recipe. Mutations happen only while creating fresh inputs.
public class ModernScalarHistoryFixture {
    private static final int INITIAL_ROWS = 20_000;
    private static final int INSERTION_STRIDE = 7_919;
    private static final int DELETE_INTERVAL = 101;
    private static final int UPDATE_INTERVAL = 97;
    private static final int APPENDED_ROWS = 100;
    private static final int BOOLEAN_COLUMNS = 9;

    private static long wide(int id) {
        return switch (id % 7) {
            case 0 -> Long.MIN_VALUE + id;
            case 1 -> Long.MAX_VALUE - id;
            case 2 -> 9_007_199_254_740_992L + id;
            case 3 -> -9_007_199_254_740_992L - id;
            case 4 -> (long) Integer.MIN_VALUE - id;
            case 5 -> (long) Integer.MAX_VALUE + id;
            default -> id * 19L;
        };
    }

    private static LocalDateTime stamp(int id) {
        LocalDateTime base = switch (id % 4) {
            case 0 -> LocalDateTime.of(1, 1, 1, 0, 0, 0);
            case 1 -> LocalDateTime.of(9999, 12, 31, 23, 59, 59);
            case 2 -> LocalDateTime.of(2000, 2, 29, 12, 34, 56);
            default -> LocalDateTime.of(2024, 2, 29, 1, 2, 3);
        };
        return base.plusNanos(id * 100L);
    }

    private static byte[] bytes(int id, boolean updated) {
        byte[] result = new byte[updated ? 4_097 : id % 67];
        for (int position = 0; position < result.length; position++) {
            result[position] = (byte) ((id + position * 17) % 256);
        }
        return result;
    }

    private static Object[] values(int id, boolean extended, boolean later, boolean updated) {
        boolean absent = id % 13 == 0;
        long number = updated ? -wide(id) : wide(id);
        String note = updated ? "grown:" + id + ":" + "é😀".repeat(128) : "n:" + id;
        List<Object> result = new ArrayList<>();
        result.add(id);
        result.add(absent ? null : number);
        result.add(absent ? null : "label:" + id + ":é中😀");
        result.add(absent ? null : note);
        result.add(absent ? null : bytes(id, updated));
        for (int bit = 0; bit < BOOLEAN_COLUMNS; bit++) {
            result.add((id & (1 << bit)) != 0);
        }
        if (extended) {
            result.add(absent ? null : stamp(id).plusNanos(updated ? 100 : 0));
        }
        if (later) {
            result.add(absent ? null : -number);
            result.add(absent ? null : (updated ? "updated:" : "later:") + id);
            result.add(id % 3 == 0);
            if (extended) {
                result.add(absent ? null : stamp(id).plusNanos(900));
            }
        }
        return result.toArray();
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        Path output = Path.of(args[0]);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported engineering format");
        };
        boolean extended = format == Database.FileFormat.V2019;

        try (Database database = new DatabaseBuilder(output).setFileFormat(format).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            TableBuilder builder = new TableBuilder("ModernHistory")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Wide", DataType.BIG_INT))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(80))
                .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
                .addColumn(new ColumnBuilder("Opaque", DataType.OLE));
            for (int bit = 0; bit < BOOLEAN_COLUMNS; bit++) {
                builder.addColumn(new ColumnBuilder("Flag" + bit, DataType.BOOLEAN));
            }
            if (extended) {
                builder.addColumn(new ColumnBuilder("Stamp", DataType.EXT_DATE_TIME));
            }
            Table table = builder
                .addIndex(new IndexBuilder("IdAsc").addColumns("ID"))
                .addIndex(new IndexBuilder("WideAsc").addColumns("Wide"))
                .toTable(database);

            for (int position = 0; position < INITIAL_ROWS; position++) {
                boolean later = position >= INITIAL_ROWS / 2;
                if (position == INITIAL_ROWS / 2) {
                    new ColumnBuilder("LaterWide", DataType.BIG_INT).addToTable(table);
                    new ColumnBuilder("LaterText", DataType.TEXT).setLengthInUnits(40).addToTable(table);
                    new ColumnBuilder("LaterFlag", DataType.BOOLEAN).addToTable(table);
                    if (extended) {
                        new ColumnBuilder("LaterStamp", DataType.EXT_DATE_TIME).addToTable(table);
                    }
                }
                int id = (int) ((long) position * INSERTION_STRIDE % INITIAL_ROWS) + 1;
                table.addRow(values(id, extended, later, false));
            }

            Cursor cursor = CursorBuilder.createCursor(table);
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                int id = row.getInt("ID");
                if (id % DELETE_INTERVAL == 0) {
                    cursor.deleteCurrentRow();
                } else if (id % UPDATE_INTERVAL == 0) {
                    cursor.updateCurrentRow(values(id, extended, true, true));
                }
            }
            for (int id = INITIAL_ROWS + 1; id <= INITIAL_ROWS + APPENDED_ROWS; id++) {
                table.addRow(values(id, extended, true, false));
            }
            int expected = INITIAL_ROWS - INITIAL_ROWS / DELETE_INTERVAL + APPENDED_ROWS;
            if (table.getRowCount() != expected) {
                throw new IllegalStateException("Unexpected retained row count");
            }
            System.out.println("Rows\t" + expected);
        }
    }
}
