import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.TimeZone;

/** One bounded public append on a fresh BinaryStorage descendant. */
public final class BinaryBoundaryAppend {
    private static final String TABLE = "BinaryStorage";
    private static final int PAYLOAD_BYTES = 255;
    private static final int FIRST_ID = 10001;
    private static final int LAST_ID = 10013;
    private static final int PATTERN_STEP = 31;

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 3) {
            throw new IllegalArgumentException("Expected source, fresh output and ID");
        }
        Path source = Path.of(arguments[0]);
        Path output = Path.of(arguments[1]);
        int identity = Integer.parseInt(arguments[2]);
        if (identity < FIRST_ID || identity > LAST_ID || Files.exists(output)) {
            throw new IllegalArgumentException("Expected planned ID and fresh output");
        }
        byte[] payload = new byte[PAYLOAD_BYTES];
        for (int position = 0; position < payload.length; position++) {
            payload[position] = (byte) (position * PATTERN_STEP + identity);
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            database.setLinkResolver((owner, token) -> {
                throw new AssertionError("No linked target is authorized");
            });
            Table table = database.getTable(TABLE);
            if (table.getColumns().size() != 2
                    || table.getColumn("ID").getType() != DataType.LONG
                    || table.getColumn("Bytes").getType() != DataType.BINARY
                    || table.getColumn("Bytes").getLength() != PAYLOAD_BYTES
                    || table.getIndexes().size() != 1) {
                throw new AssertionError("Expected ordinary BINARY255 schema");
            }
            var primary = table.getIndexes().get(0);
            if (!primary.getName().equals("PrimaryID") || !primary.isPrimaryKey()
                    || !primary.isUnique() || !primary.isRequired()
                    || primary.shouldIgnoreNulls() || primary.getColumns().size() != 1
                    || !primary.getColumns().get(0).getName().equals("ID")) {
                throw new AssertionError("Expected retained ordinary ID primary");
            }
            int count = table.getRowCount();
            table.addRow(identity, payload);
            if (table.getRowCount() != count + 1) {
                throw new AssertionError("Append must add exactly one row");
            }
            System.out.printf("APPEND\t%s\t%d\t%d\t%d\t%d%n",
                TABLE, identity, payload.length, count, table.getRowCount());
        }
    }
}
