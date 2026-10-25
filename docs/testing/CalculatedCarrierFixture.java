import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert cached-carrier corpus; formulas never execute. */
public class CalculatedCarrierFixture {
    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        Path path = Path.of(arguments[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh path required");
        }
        Database.FileFormat format = switch (arguments[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            for (DataType type : new DataType[] {
                DataType.BYTE, DataType.INT, DataType.BIG_INT, DataType.FLOAT, DataType.GUID
            }) {
                Table table = builder("Carrier" + type, type).toTable(database);
                builder("EmptyCarrier" + type, type).toTable(database);
                Object[] values = switch (type) {
                    case BYTE -> new Object[] {0, 255, 128, null, 1, 17, 231};
                    case INT -> new Object[] {Short.MIN_VALUE, Short.MAX_VALUE, -1, null, 0, 1234, -23456};
                    case BIG_INT -> new Object[] {
                        Long.MIN_VALUE, Long.MAX_VALUE, -1L, null, 0L, 9007199254740993L, -234567890123456789L
                    };
                    case FLOAT -> new Object[] {
                        -0.0f, Float.MIN_VALUE, Float.MAX_VALUE, null, -Float.MAX_VALUE,
                        1.25f, -17.75f
                    };
                    case GUID -> new Object[] {
                        "{00000000-0000-0000-0000-000000000000}",
                        "{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}",
                        "{01234567-89AB-CDEF-0123-456789ABCDEF}", null,
                        "{80000000-0001-0002-0003-000000000004}",
                        "{DEADBEEF-CAFE-FEED-0102-030405060708}",
                        "{ABCDEF01-2345-6789-ABCD-EF0123456789}"
                    };
                    default -> throw new AssertionError();
                };
                for (int ordinal = 0; ordinal < values.length; ordinal++) {
                    table.addRow(ordinal + 1, values[ordinal]);
                }
            }
            Table exceptional = builder("ExceptionalFLOAT", DataType.FLOAT).toTable(database);
            builder("EmptyExceptionalFLOAT", DataType.FLOAT).toTable(database);
            Object[] values = {
                -0.0f, Float.MIN_VALUE, Float.POSITIVE_INFINITY, null,
                Float.NEGATIVE_INFINITY, Float.NaN, Float.intBitsToFloat(0xffc01234)
            };
            for (int ordinal = 0; ordinal < values.length; ordinal++) {
                exceptional.addRow(ordinal + 1, values[ordinal]);
            }
        }
    }

    private static TableBuilder builder(String name, DataType type) {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Cached", type).setCalculatedInfo("[ID]*2"));
    }
}
