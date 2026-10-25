import com.healthmarketscience.jackcess.Column;
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
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.TimeZone;

/** Original inert fixture recipe and read-only retained-DATE getter. */
public final class DateCandidateFixture {
    private static final int ROW_COUNT = 20_000;
    private static final int ID_MULTIPLIER = 137;
    private static final int NULL_PERIOD = 11;
    private static final int SERIAL_MULTIPLIER = 7;
    private static final String TABLE = "DateCandidates";

    private static Object retainedDate(Column column, long bits) throws Exception {
        byte[] bytes = ByteBuffer.allocate(Long.BYTES)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putLong(bits)
            .array();
        return ((ColumnImpl) column).read(bytes);
    }

    private static void configure(Database database) {
        database.setEvaluateExpressions(false);
        database.setDateTimeType(DateTimeType.DATE);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
    }

    private static void create(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }
        Database.FileFormat format = switch (profile) {
            case "V2000" -> Database.FileFormat.V2000;
            case "V2007" -> Database.FileFormat.V2007;
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        long[] serials = {
            0,
            0x8000000000000000L,
            1,
            0x8000000000000001L,
            Double.doubleToRawLongBits(0.25),
            Double.doubleToRawLongBits(-0.25),
            Double.doubleToRawLongBits(-1.25),
            Double.doubleToRawLongBits(-1.75),
            Double.doubleToRawLongBits(1.0000000001),
            Double.doubleToRawLongBits(-1.0000000001),
            Double.doubleToRawLongBits(45000.125),
            Double.doubleToRawLongBits(-657434.25),
            Double.doubleToRawLongBits(2958465.9999999995),
        };

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(format)
                .setAutoSync(false)
                .create()) {
            configure(database);
            Table table = new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Moment", DataType.SHORT_DATE_TIME))
                .addIndex(new IndexBuilder("DateAsc").addColumns("Moment"))
                .addIndex(new IndexBuilder("DateDesc").addColumns(false, "Moment"))
                .toTable(database);
            Column moment = table.getColumn("Moment");

            for (int position = 0; position < ROW_COUNT; position++) {
                int identifier = (position * ID_MULTIPLIER) % ROW_COUNT;
                Object value = position % NULL_PERIOD == 0
                    ? null
                    : retainedDate(moment, serials[(position * SERIAL_MULTIPLIER) % serials.length]);
                table.addRow(identifier, value);
            }
            if (table.getRowCount() != ROW_COUNT) {
                throw new AssertionError("Incomplete fixture");
            }
        }
    }

    private static void read(Path path, String order) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            configure(database);
            Table table = database.getTable(TABLE);
            ColumnImpl moment = (ColumnImpl) table.getColumn("Moment");
            Cursor cursor = order.equals("table")
                ? CursorBuilder.createCursor(table)
                : CursorBuilder.createCursor(table.getIndex(order));
            Row row;

            while ((row = cursor.getNextRow()) != null) {
                Object value = row.get("Moment");
                if (value == null) {
                    System.out.println(row.get("ID") + "\tNULL");
                    continue;
                }
                // Serialize the retained carrier. Calendar conversion and
                // toDateDouble would lose valid sub-millisecond payload bits.
                ByteBuffer bytes = moment.write(value, 0, ByteOrder.LITTLE_ENDIAN);
                if (bytes.limit() != Long.BYTES) {
                    throw new IllegalArgumentException("Unexpected DATE width");
                }
                System.out.printf("%s\t%016x%n", row.get("ID"), bytes.getLong(0));
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 3) {
            throw new IllegalArgumentException("Expected create/read, path and profile/order");
        }
        Path path = Path.of(arguments[1]);
        switch (arguments[0]) {
            case "create" -> create(path, arguments[2]);
            case "read" -> read(path, arguments[2]);
            default -> throw new IllegalArgumentException("Unknown operation");
        }
    }
}
