import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

// Original inert recipe; creation and mutations are confined to fresh files.
public class WideTableHistoryFixture {
    private static final int MAX_COLUMNS = 255;
    private static final int[] WIDTHS = {8, 9, 16, 17, 127, 128, 129, 254, MAX_COLUMNS};
    private static final String[] TEXT_VALUES = {"", "a", "é", "中", "😀"};

    private static String name(int column) {
        return String.format(java.util.Locale.ROOT, "C%03d", column);
    }

    private static DataType type(int column, boolean mixed) {
        if (!mixed) {
            return DataType.TEXT;
        }
        return switch (column % 5) {
            case 0 -> DataType.BOOLEAN;
            case 1 -> DataType.BYTE;
            case 2 -> DataType.INT;
            case 3 -> DataType.LONG;
            default -> DataType.TEXT;
        };
    }

    private static ColumnBuilder column(int position, boolean mixed) {
        ColumnBuilder result = new ColumnBuilder(name(position), type(position, mixed));
        if (type(position, mixed) == DataType.TEXT) {
            result.setLengthInUnits(8);
        }
        return result;
    }

    private static Object[] values(int id, int width, boolean mixed, boolean updated) {
        Object[] result = new Object[width];
        result[0] = id;
        for (int position = 1; position < width; position++) {
            if ((id + position) % 7 == 0) {
                continue;
            }
            result[position] = switch (type(position, mixed)) {
                case BOOLEAN -> (id + position) % 2 == 0;
                case BYTE -> (byte) ((id + position) % 256);
                case INT -> (short) (id * 100 - position);
                case LONG -> id * 100_000 - position;
                case TEXT -> updated ? "é😀" : TEXT_VALUES[(id + position) % TEXT_VALUES.length];
                default -> throw new IllegalStateException("Unexpected recipe type");
            };
        }
        return result;
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || Files.exists(Path.of(args[0]))) {
            throw new IllegalArgumentException("Expected fresh path and format");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2000" -> Database.FileFormat.V2000;
            case "V2007" -> Database.FileFormat.V2007;
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported recipe format");
        };

        try (Database database = new DatabaseBuilder(Path.of(args[0]))
                .setFileFormat(format).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            for (boolean mixed : new boolean[] {false, true}) {
                String tableName = mixed ? "WideMixed" : "WideVariable";
                Table table = new TableBuilder(tableName)
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addIndex(new IndexBuilder("IdAsc").addColumns("ID"))
                    .toTable(database);
                int identity = 1;
                for (int width : WIDTHS) {
                    while (table.getColumnCount() < width) {
                        column(table.getColumnCount(), mixed).addToTable(table);
                    }
                    for (int row = 0; row < 2; row++) {
                        table.addRow(values(identity++, width, mixed, false));
                    }
                }

                Cursor cursor = CursorBuilder.createCursor(table);
                Row row;
                while ((row = cursor.getNextRow()) != null) {
                    int id = row.getInt("ID");
                    if (id == 6 || id == 17) {
                        cursor.deleteCurrentRow();
                    } else if (id == 1) {
                        cursor.updateCurrentRow(values(id, MAX_COLUMNS, mixed, true));
                    }
                }
                for (int id = identity; id < identity + 2; id++) {
                    table.addRow(values(id, MAX_COLUMNS, mixed, false));
                }
                if (table.getColumnCount() != MAX_COLUMNS || table.getRowCount() != 18) {
                    throw new IllegalStateException("Unexpected retained recipe shape");
                }
                System.out.println(tableName + "\t" + table.getRowCount());
            }
        }
    }
}
