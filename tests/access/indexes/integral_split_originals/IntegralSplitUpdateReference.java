import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.math.BigDecimal;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.List;
import java.util.TimeZone;

/** Exact public row setter on a fresh bounded copy; no header or flag changes. */
public final class IntegralSplitUpdateReference {
    private static final long MAX_BYTES = 64L * 1024 * 1024;
    private static final int SELECTED_ID = 1;
    private static final int EXPECTED_MARKER = 17;
    private static final String KEY = "KeyValue";

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked target is authorized");
        });
    }

    private static void properties(Table table) throws Exception {
        for (PropertyMap.Property property : table.getProperties()) {
            throw new IllegalArgumentException("Unexpected table property: " + property.getName());
        }
        for (Column column : table.getColumns()) {
            boolean required = List.of("ID", KEY).contains(column.getName());
            int count = 0;
            for (PropertyMap.Property property : column.getProperties()) {
                if (!required || !property.getName().equals("Required")
                        || property.getType() != DataType.BOOLEAN || !property.isDdl()
                        || !Boolean.TRUE.equals(property.getValue())) {
                    throw new IllegalArgumentException("Unqualified field property");
                }
                count++;
            }
            if (count != (required ? 1 : 0)) {
                throw new IllegalArgumentException("Required property does not match the frozen source");
            }
        }
    }

    private static Object replacement(DataType type, Object before) {
        return switch (type) {
            case BYTE -> {
                if (!(before instanceof Byte value) || Byte.toUnsignedInt(value) != 0) {
                    throw new IllegalArgumentException("Expected exact unsigned Byte zero");
                }
                yield (byte) 255;
            }
            case INT -> {
                if (!(before instanceof Short value) || value != 0) {
                    throw new IllegalArgumentException("Expected exact Int16 zero");
                }
                yield Short.MAX_VALUE;
            }
            case BIG_INT -> {
                if (!(before instanceof Long value) || value != 0) {
                    throw new IllegalArgumentException("Expected exact Int64 zero");
                }
                yield Long.MAX_VALUE;
            }
            case MONEY -> {
                if (!(before instanceof BigDecimal value) || value.scale() != 4
                        || value.signum() != 0) {
                    throw new IllegalArgumentException("Expected exact scale-four Currency zero");
                }
                yield BigDecimal.valueOf(Long.MAX_VALUE, 4);
            }
            default -> throw new IllegalArgumentException("Unqualified native target type");
        };
    }

    private static void update(Path source, Path output, String target) throws Exception {
        if (!List.of("ByteKeys", "Int16Keys", "Int64Keys", "CurrencyKeys").contains(target)
                || Files.exists(output) || Files.size(source) > MAX_BYTES) {
            throw new IllegalArgumentException("Frozen target and fresh bounded output required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).setAutoSync(false).open()) {
            inert(database);
            Table table = database.getTable(target);
            if (table.getColumnCount() != 3) {
                throw new IllegalArgumentException("Unexpected target column count");
            }
            properties(table);
            DataType expectedType = switch (target) {
                case "ByteKeys" -> DataType.BYTE;
                case "Int16Keys" -> DataType.INT;
                case "Int64Keys" -> DataType.BIG_INT;
                case "CurrencyKeys" -> DataType.MONEY;
                default -> throw new IllegalArgumentException("Unqualified target");
            };
            if (table.getColumn(KEY).getType() != expectedType) {
                throw new IllegalArgumentException("Declared type does not match the frozen target");
            }
            Row selected = null;
            for (Row row : table) {
                if (row.getInt("ID") == SELECTED_ID) {
                    selected = row;
                    break;
                }
            }
            if (selected == null || selected.getInt("Marker") != EXPECTED_MARKER) {
                throw new IllegalArgumentException("Original selected row or marker changed");
            }
            Object value = replacement(table.getColumn(KEY).getType(), selected.get(KEY));
            selected.put(KEY, value);
            table.updateRow(selected);
            database.flush();
            System.out.printf("UPDATED\t%s\t%d\t%s\t%s%n", target, SELECTED_ID,
                table.getColumn(KEY).getType(), value);
        }
        if (Files.size(output) > MAX_BYTES) {
            throw new AssertionError("Bounded copy exceeded the frozen file ceiling");
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 3) {
            throw new IllegalArgumentException("Expected SOURCE FRESH_OUTPUT TABLE");
        }
        update(Path.of(arguments[0]), Path.of(arguments[1]), arguments[2]);
    }
}
