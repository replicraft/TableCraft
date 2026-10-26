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
import com.healthmarketscience.jackcess.impl.IndexImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;

/** Original larger version of the repository-owned UsageMapFixture geometry. */
public final class LargeUsageMapProbe {
    private static final int ROWS = 5000;
    private static final int BINARY_FIELDS = 12;
    private static final int BINARY_BYTES = 240;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, name) -> {
            throw new AssertionError("No linked handles authorized");
        });
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh path required");
        }
        if (!profile.equals("V2016") && !profile.equals("V2019")) {
            throw new IllegalArgumentException("ACE16 priority profiles only");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            TableBuilder builder = new TableBuilder("Pages")
                .addColumn(new ColumnBuilder("ID", DataType.LONG));
            for (int column = 0; column < BINARY_FIELDS; column++) {
                builder.addColumn(new ColumnBuilder("B" + column, DataType.BINARY).setLength(BINARY_BYTES));
            }
            Table table = builder.addIndex(new IndexBuilder("PrimaryKey")
                .addColumns("ID").setPrimaryKey()).toTable(database);
            for (int row = 0; row < ROWS; row++) {
                Object[] values = new Object[BINARY_FIELDS + 1];
                values[0] = row;
                for (int column = 0; column < BINARY_FIELDS; column++) {
                    byte[] bytes = new byte[BINARY_BYTES];
                    for (int position = 0; position < bytes.length; position++) {
                        bytes[position] = (byte) (row + column + position);
                    }
                    values[column + 1] = bytes;
                }
                table.addRow(values);
            }
        }
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Table table = database.getTable("Pages");
            TableImpl nativeTable = (TableImpl) table;
            System.out.printf("TABLE\t%d\t%d\t%d%n", table.getRowCount(),
                nativeTable.getTableDefPageNumber(), nativeTable.getFormat().PAGE_SIZE);
            var pages = nativeTable.getOwnedPagesCursor();
            int page;
            int count = 0;
            while ((page = pages.getNextPage()) >= 0) {
                System.out.printf("PAGE\t%d%n", page);
                count++;
            }
            System.out.printf("OWNED\t%d%n", count);
            for (Row row : table) {
                MessageDigest hash = MessageDigest.getInstance("SHA-256");
                for (int column = 0; column < BINARY_FIELDS; column++) {
                    byte[] bytes = row.getBytes("B" + column);
                    if (bytes.length != BINARY_BYTES) {
                        throw new AssertionError("Unexpected stored Binary width");
                    }
                    hash.update(bytes);
                }
                RowIdImpl identity = (RowIdImpl) row.getId();
                System.out.printf("ROW\t%d\t%d\t%d\t%s%n", row.getInt("ID"),
                    identity.getPageNumber(), identity.getRowNumber(),
                    HexFormat.of().formatHex(hash.digest()));
            }
            IndexImpl index = (IndexImpl) table.getIndex("PrimaryKey");
            System.out.printf("INDEX\t%d\t%d%n", index.getIndexData().getEntryCount(),
                index.getIndexData().getOwnedPageCount());
            Cursor cursor = CursorBuilder.createCursor(index);
            Row row;
            while ((row = cursor.getNextRow()) != null) {
                System.out.printf("ENTRY\t%d%n", row.getInt("ID"));
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE or oracle PATH");
        }
    }
}
