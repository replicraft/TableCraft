import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.util.Locale;

// Original inert recipe. Native data changes occur only during fresh generation.
public class MixedScalarHistoryFixture {
    private static final int INITIAL_ROWS = 20_000;
    private static final int INSERTION_STRIDE = 7_919;
    private static final int DELETE_INTERVAL = 101;
    private static final int UPDATE_INTERVAL = 97;
    private static final int APPENDED_ROWS = 100;

    private static byte[] bytes(int id, int length) {
        byte[] result = new byte[length];
        for (int position = 0; position < length; position++) {
            result[position] = (byte) ((id + position * 17) % 256);
        }
        return result;
    }

    private static Object[] values(int id, boolean later, boolean updated) {
        boolean absent = id % 13 == 0;
        int signed = id % 2 == 0 ? id : -id;
        String notes = updated ? "grown:" + id + ":" + "é😀".repeat(256) : "n:" + id;
        String guid = String.format(Locale.ROOT, "{%08x-1234-5678-9abc-%012x}", id, id * 17L);
        LocalDateTime stamp = id % 2 == 0
            ? LocalDateTime.of(2024, 1, 1, 12, 0).plusDays(id % 365)
            : LocalDateTime.of(1899, 12, 29, 6, 0);
        Object[] result = new Object[later ? 17 : 14];
        result[0] = id;
        result[1] = id % 3 == 0;
        result[2] = absent ? null : (byte) (id % 256);
        result[3] = absent ? null : (short) (id % 65_536 - 32_768);
        result[4] = absent ? null : BigDecimal.valueOf(signed * 10_001L, 4);
        result[5] = absent ? null : (id % 17 == 0 ? -0.0f : signed / 8.0f);
        result[6] = absent ? null : (id % 19 == 0 ? -0.0d : signed / 16.0d);
        result[7] = absent ? null : "label:" + id + ":é中😀";
        result[8] = absent ? null : notes;
        result[9] = absent ? null : bytes(id, id % 17);
        result[10] = absent ? null : bytes(id, updated ? 8_193 : id % 67);
        result[11] = absent ? null : guid;
        result[12] = absent ? null : new BigDecimal("900719925474099300000000")
            .add(BigDecimal.valueOf(signed)).movePointLeft(4);
        result[13] = absent ? null : stamp;
        if (later) {
            result[14] = absent ? null : id * 2;
            result[15] = absent ? null : (updated ? "updated:" : "later:") + id;
            result[16] = absent ? null : BigDecimal.valueOf(-signed * 3L, 4);
        }
        return result;
    }

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
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            Table table = new TableBuilder("MixedHistory")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN))
                .addColumn(new ColumnBuilder("Octet", DataType.BYTE))
                .addColumn(new ColumnBuilder("Small", DataType.INT))
                .addColumn(new ColumnBuilder("Money", DataType.MONEY))
                .addColumn(new ColumnBuilder("Real", DataType.FLOAT))
                .addColumn(new ColumnBuilder("WideReal", DataType.DOUBLE))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(80))
                .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
                .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(16))
                .addColumn(new ColumnBuilder("Opaque", DataType.OLE))
                .addColumn(new ColumnBuilder("G", DataType.GUID))
                .addColumn(new ColumnBuilder("Amount", DataType.NUMERIC).setPrecision(28).setScale(4))
                .addColumn(new ColumnBuilder("Stamp", DataType.SHORT_DATE_TIME))
                .toTable(database);

            for (int position = 0; position < INITIAL_ROWS; position++) {
                boolean later = position >= INITIAL_ROWS / 2;
                if (position == INITIAL_ROWS / 2) {
                    new ColumnBuilder("Later", DataType.LONG).addToTable(table);
                    new ColumnBuilder("LaterText", DataType.TEXT).setLengthInUnits(40).addToTable(table);
                    new ColumnBuilder("LaterMoney", DataType.MONEY).addToTable(table);
                }
                int id = (int) ((long) position * INSERTION_STRIDE % INITIAL_ROWS) + 1;
                table.addRow(values(id, later, false));
            }

            Cursor cursor = CursorBuilder.createCursor(table);
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                int id = row.getInt("ID");
                if (id % DELETE_INTERVAL == 0) {
                    cursor.deleteCurrentRow();
                } else if (id % UPDATE_INTERVAL == 0) {
                    cursor.updateCurrentRow(values(id, true, true));
                }
            }
            for (int id = INITIAL_ROWS + 1; id <= INITIAL_ROWS + APPENDED_ROWS; id++) {
                table.addRow(values(id, true, false));
            }
            int expected = INITIAL_ROWS - INITIAL_ROWS / DELETE_INTERVAL + APPENDED_ROWS;
            if (table.getRowCount() != expected) {
                throw new IllegalStateException("Unexpected retained row count");
            }
            System.out.println("Rows\t" + expected);
        }
    }
}
