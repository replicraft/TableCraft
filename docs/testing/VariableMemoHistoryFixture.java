import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.LinkedHashMap;

/** Original inert wide-variable/Memo history generator and reopened read-only oracle. */
public final class VariableMemoHistoryFixture {
    private static String name(int position) {
        return String.format("V%03d", position);
    }

    private static ColumnBuilder column(int position) {
        return position % 31 == 0 ? new ColumnBuilder(name(position), DataType.MEMO)
            : new ColumnBuilder(name(position), DataType.TEXT).setLengthInUnits(16);
    }

    private static void write(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            database.setEvaluateExpressions(false);
            TableBuilder builder = new TableBuilder("VariableHistory")
                .addColumn(new ColumnBuilder("ID", DataType.LONG));
            for (int index = 0; index < 16; index++) {
                builder.addColumn(column(index));
            }
            Table table = builder.toTable(database);
            int previous = 16;
            int identity = 1;
            for (int width : new int[] {16, 127, 128, 254}) {
                for (int index = previous; index < width; index++) {
                    column(index).addToTable(table);
                }
                previous = width;
                for (int row = 0; row < 8; row++) {
                    LinkedHashMap<String, Object> cells = new LinkedHashMap<>();
                    cells.put("ID", identity);
                    for (int index = 0; index < width; index++) {
                        cells.put(name(index), index % 7 == 0 ? null : "r" + identity + "v" + index);
                    }
                    table.addRowFromMap(cells);
                    identity++;
                }
            }

            for (Row row : table) {
                int id = row.getInt("ID");
                if ((id - 1) % 8 == 0) {
                    continue;
                }
                row.put("V000", "é🦀".repeat(id % 2 == 0 ? 4000 : 3));
                row.put("V126", "late" + id);
                row.put("V127", id % 3 == 0 ? null : "after" + id);
                row.put("V253", "tail" + id);
                table.updateRow(row);
            }
            new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey().addToTable(table);
        }
    }

    private static void read(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked opens");
            });
            Table table = database.getTable("VariableHistory");
            System.out.println("COLUMNS\t" + table.getColumnCount());
            for (Column column : table.getColumns()) {
                System.out.println("COLUMN\t" + column.getName() + "\t" + column.getType());
            }
            TableImpl nativeTable = (TableImpl) table;
            if (nativeTable.getFormat().SIZE_ROW_COLUMN_COUNT != Short.BYTES) {
                throw new IllegalStateException("Unqualified row-header width");
            }

            for (Row row : table) {
                RowIdImpl location = (RowIdImpl) row.getId();
                var buffer = TableImpl.positionAtRowData(nativeTable.createRowState(), location)
                    .order(ByteOrder.LITTLE_ENDIAN);
                int count = Short.toUnsignedInt(buffer.getShort());
                System.out.println("PHYSICAL\t" + row.getInt("ID") + "\t" + count
                    + "\t" + location.getPageNumber() + "\t" + location.getRowNumber());
                StringBuilder output = new StringBuilder("ROW\t").append(row.getInt("ID"));
                for (Column column : table.getColumns()) {
                    if (column.getName().equals("ID")) {
                        continue;
                    }
                    String value = row.getString(column.getName());
                    output.append('\t').append(value == null ? "N" : "T" + Base64.getEncoder()
                        .encodeToString(value.getBytes(StandardCharsets.UTF_8)));
                }
                System.out.println(output);
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new IllegalStateException("Unexpected linked handles");
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
