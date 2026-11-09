import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.JetFormat;
import com.healthmarketscience.jackcess.impl.PageChannel;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

/** SDK-created ORIGINAL only; no inserted-request AFTER or TableCraft writer. */
public final class BuildAssociationOriginal {
    private static final Set<String> PROFILES = Set.of("V2016", "V2019");
    private static final long FILE_CEILING = 8L * 1024 * 1024;
    private static final int DATA_KIND = 1;
    private static final int DATA_OWNER_OFFSET = 4;
    private static final int OLD_ROWS = 9;
    private static final int OLD_TEXT_UNITS = 230;
    private static final int COMPLETE_ROW_OVERHEAD = 21;
    private static final int SHORT_TEXT_UNITS = 3;
    private static final int LONG_TEXT_UNITS = 200;
    private static final int UTF16_UNIT_BYTES = Character.BYTES;

    private record Capacity(int page, int count, int free) { }

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new IllegalStateException(message);
        }
    }

    private static List<Capacity> capacities(TableImpl table) throws Exception {
        PageChannel channel = table.getPageChannel();
        JetFormat format = table.getFormat();
        UsageMap.PageCursor cursor = table.getOwnedPagesCursor();
        List<Capacity> result = new ArrayList<>();
        int previous = -1;
        int number;
        while ((number = cursor.getNextPage()) >= 0) {
            require(number > previous, "Owned pages must be ascending");
            previous = number;
            ByteBuffer page = channel.createPageBuffer();
            channel.readPage(page, number);
            require(Byte.toUnsignedInt(page.get(0)) == DATA_KIND,
                "Every target-owned page must be DATA");
            require(page.getInt(DATA_OWNER_OFFSET) == table.getTableDefPageNumber(),
                "DATA owner must be the actual Items definition");
            int count = Short.toUnsignedInt(page.getShort(format.OFFSET_NUM_ROWS_ON_DATA_PAGE));
            int free = Short.toUnsignedInt(page.getShort(format.OFFSET_FREE_SPACE));
            for (int slot = 0; slot < count; slot++) {
                int start = TableImpl.findRowStart(page, slot, format);
                int end = TableImpl.findRowEnd(page, slot, format);
                require(end - start == COMPLETE_ROW_OVERHEAD + UTF16_UNIT_BYTES * OLD_TEXT_UNITS,
                    "Every original row must match its uncompressed literal width");
            }
            result.add(new Capacity(number, count, free));
        }
        return result;
    }

    private static UsageMap candidates(TableImpl table) throws Exception {
        ByteBuffer definition = table.getPageChannel().createPageBuffer();
        table.getPageChannel().readPage(definition, table.getTableDefPageNumber());
        definition.position(table.getFormat().OFFSET_FREE_SPACE_PAGES);
        return UsageMap.read(table.getDatabase(), definition);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || !PROFILES.contains(args[1])) {
            throw new IllegalArgumentException("Expected absent PATH V2016|V2019");
        }
        Path target = Path.of(args[0]);
        require(!Files.exists(target), "Original path must be absent");

        try (FileChannel raw = FileChannel.open(target, StandardOpenOption.CREATE_NEW,
                StandardOpenOption.READ, StandardOpenOption.WRITE);
             BoundedFileChannel bounded = new BoundedFileChannel(raw, FILE_CEILING);
             Database database = new DatabaseBuilder(target.toFile())
                .setChannel(bounded)
                .setFileFormat(Database.FileFormat.valueOf(args[1]))
                .create()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("External databases are outside this recipe");
            });
            Table items = new TableBuilder("Items")
                .addColumn(new ColumnBuilder("Id", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Score", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Guard", DataType.LONG).setAutoNumber(false))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT)
                    .setLengthInUnits(255).setCompressedUnicode(false))
                .addIndex(new IndexBuilder("PrimaryID").addColumns("Id").setPrimaryKey())
                .addIndex(new IndexBuilder("PrimaryAlias").addColumns("Id")
                    .setRequired().setUnique())
                .addIndex(new IndexBuilder("GroupOrder").addColumns("Guard"))
                .addIndex(new IndexBuilder("GroupAlias").addColumns("Guard"))
                .toTable(database);
            for (int id = 1; id <= OLD_ROWS; id++) {
                items.addRow(id, id - 5, (id - 1) % 3, "o".repeat(OLD_TEXT_UNITS));
            }
            Table sentinel = new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("Marker", DataType.LONG).setAutoNumber(false))
                .toTable(database);
            sentinel.addRow(999);
            database.flush();

            TableImpl nativeItems = (TableImpl) items;
            List<Capacity> pages = capacities(nativeItems);
            require(pages.size() == 2, "The frozen recipe must create exactly two owned DATA pages");
            Capacity early = pages.get(0);
            Capacity late = pages.get(1);
            require(early.count() == 8 && late.count() == 1,
                "Observe the intended original eight/one row partition");
            int shortNeeded = COMPLETE_ROW_OVERHEAD + UTF16_UNIT_BYTES * SHORT_TEXT_UNITS
                + Short.BYTES;
            int longNeeded = COMPLETE_ROW_OVERHEAD + UTF16_UNIT_BYTES * LONG_TEXT_UNITS
                + Short.BYTES;
            require(shortNeeded <= early.free() && early.free() < longNeeded
                    && longNeeded <= late.free(),
                "Observe short-fit/long-skip on early page and long-fit on late page");

            // Jackcess clears an earlier page's approximate free-space hint
            // after a larger row fails to fit. Restore only that hint using
            // its own UsageMap API. Ownership/GLOBAL/rows are never patched.
            UsageMap hint = candidates(nativeItems);
            require(hint.containsPageNumber(late.page()), "Late page must remain a candidate");
            boolean earlyBefore = hint.containsPageNumber(early.page());
            PageChannel channel = nativeItems.getPageChannel();
            channel.startWrite();
            try {
                hint.addPageNumber(early.page());
            } finally {
                channel.finishWrite();
            }
            database.flush();
            UsageMap observed = candidates(nativeItems);
            require(observed.containsPageNumber(early.page())
                    && observed.containsPageNumber(late.page()),
                "Both existing DATA pages must be explicit candidate members");
            require(capacities(nativeItems).equals(pages), "Hint restoration must not change DATA geometry");
            require(Files.size(target) <= FILE_CEILING, "Original exceeds the bounded channel ceiling");

            System.out.printf("CREATED\t%s\tItems\t%d\t%d%n",
                args[1], items.getRowCount(), nativeItems.getTableDefPageNumber());
            System.out.printf("EARLY\t%d\t%d\t%d\t%s\ttrue%n",
                early.page(), early.count(), early.free(), earlyBefore);
            System.out.printf("LATE\t%d\t%d\t%d\ttrue%n",
                late.page(), late.count(), late.free());
            System.out.printf("REQUEST_CAPACITY\t%d\t%d%n", longNeeded, shortNeeded);
        }
    }
}
