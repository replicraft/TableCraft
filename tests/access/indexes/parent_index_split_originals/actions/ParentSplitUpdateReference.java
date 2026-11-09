import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.TimeZone;

/** One public UPDATE on a fresh sparse source copy; never generates inputs. */
public final class ParentSplitUpdateReference {
    private static final int EXPECTED = 99424;
    private static final int REPLACEMENT = Integer.MAX_VALUE;
    private static final int LIVE_ROWS = 782;
    private static final int ORDINAL = 781;
    private static final int EXPECTED_MARKER = 298272;
    private static final long MAX_BYTES = 64L * 1024 * 1024;

    private static String hash(Path path) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (var input = Files.newInputStream(path)) {
            byte[] buffer = new byte[64 * 1024];
            int count;
            while ((count = input.read(buffer)) != -1) {
                digest.update(buffer, 0, count);
            }
        }
        return HexFormat.of().formatHex(digest.digest());
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 5) {
            throw new IllegalArgumentException("Expected SOURCE FRESH_OUTPUT SHA256 PAGE SLOT");
        }
        Path source = Path.of(arguments[0]);
        Path output = Path.of(arguments[1]);
        String expectedHash = arguments[2];
        int expectedPage = Integer.parseInt(arguments[3]);
        int expectedSlot = Integer.parseInt(arguments[4]);
        if (Files.exists(output) || !hash(source).equals(expectedHash)
                || Files.size(source) > MAX_BYTES) {
            throw new IllegalArgumentException("Expected immutable bounded source and fresh output");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).open()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            database.setLinkResolver((ignored, token) -> {
                throw new AssertionError("No linked target is authorized");
            });
            if (!database.isEnforceForeignKeys() || !database.getRelationships().isEmpty()) {
                throw new AssertionError("Unexpected relationship/enforcement state");
            }
            for (String name : List.of("SparseInt32", "Sentinel")) {
                var table = database.getTable(name);
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                for (var column : table.getColumns()) {
                    if (column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected column properties");
                    }
                }
            }
            var table = database.getTable("SparseInt32");
            var primary = table.getIndex("PrimaryKey");
            if (table.getRowCount() != LIVE_ROWS || !primary.isPrimaryKey()
                    || !primary.isRequired() || !primary.isUnique() || primary.shouldIgnoreNulls()) {
                throw new AssertionError("Expected bounded required unique Int32 source");
            }
            Row selected = null;
            int ordinal = 0;
            for (Row row : table) {
                int value = row.getInt("Key");
                if (value == REPLACEMENT) {
                    throw new AssertionError("Replacement must be unused");
                }
                if (value == EXPECTED) {
                    RowIdImpl reference = (RowIdImpl) row.getId();
                    if (selected != null || ordinal != ORDINAL
                            || row.getInt("Marker") != EXPECTED_MARKER
                            || reference.getPageNumber() != expectedPage
                            || reference.getRowNumber() != expectedSlot) {
                        throw new AssertionError("Selected original identity/precondition differs");
                    }
                    selected = row;
                }
                ordinal++;
            }
            if (selected == null || ordinal != LIVE_ROWS) {
                throw new AssertionError("Selected original row missing");
            }
            selected.put("Key", REPLACEMENT);
            table.updateRow(selected);
            System.out.println("updated=1; foreignKeys=true; expressions=false; timezone=UTC");
        }
        if (!hash(source).equals(expectedHash)) {
            throw new AssertionError("Original sparse source changed");
        }
    }
}
