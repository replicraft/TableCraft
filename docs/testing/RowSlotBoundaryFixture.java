import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.Set;

/** Fresh one-LONG-column public-writer page/slot boundary observations. */
public final class RowSlotBoundaryFixture {
    private static final int ROWS = 600;
    private static final String TABLE = "Slots";

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked target is authorized");
        });
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table table = new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("N", DataType.LONG)).toTable(database);
            for (int index = 0; index < ROWS; index++) {
                table.addRow(index);
            }
        }
    }

    private static void oracle(Path path) throws Exception {
        byte[] image = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            TableImpl table = (TableImpl) database.getTable(TABLE);
            var format = table.getFormat();
            System.out.printf("FORMAT\t%d\t%d\t%d\t%d\t%d\t%d%n", format.PAGE_SIZE,
                format.OFFSET_NUM_ROWS_ON_DATA_PAGE, format.OFFSET_ROW_START,
                format.SIZE_ROW_LOCATION, format.MAX_NUM_ROWS_ON_DATA_PAGE, format.MAX_ROW_SIZE);
            System.out.printf("TABLE\t%s\t%d\t%d\t%d%n", TABLE, table.getTableDefPageNumber(),
                table.getRowCount(), table.getIndexes().size());
            Set<Integer> pages = new HashSet<>();
            int count = 0;
            int maximumSlot = -1;
            for (Row row : table) {
                RowIdImpl location = (RowIdImpl) row.getId();
                int pageNumber = location.getPageNumber();
                int slot = location.getRowNumber();
                int pageStart = pageNumber * format.PAGE_SIZE;
                ByteBuffer page = ByteBuffer.wrap(image, pageStart, format.PAGE_SIZE)
                    .slice().order(ByteOrder.LITTLE_ENDIAN);
                int storedCount = Short.toUnsignedInt(page.getShort(format.OFFSET_NUM_ROWS_ON_DATA_PAGE));
                int word = Short.toUnsignedInt(page.getShort(
                    format.OFFSET_ROW_START + slot * format.SIZE_ROW_LOCATION));
                if (pages.add(pageNumber)) {
                    System.out.printf("PAGE\t%d\t%d\t%d%n", pageNumber, storedCount, page.getInt(4));
                }
                System.out.printf("ROW\t%d\t%d\t%d\t%d\t%d%n", row.getInt("N"),
                    pageNumber, slot, storedCount, word);
                count++;
                maximumSlot = Math.max(maximumSlot, slot);
            }
            if (count != ROWS || count != table.getRowCount() || !table.getIndexes().isEmpty()) {
                throw new AssertionError("Original count/no-index inventory differs");
            }
            System.out.printf("SUMMARY\t%d\t%d\t%d%n", count, pages.size(), maximumSlot);
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate NEW_PATH FORMAT or oracle PATH");
        }
    }
}
