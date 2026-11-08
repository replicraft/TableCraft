import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.io.File;
import java.math.BigDecimal;
import java.util.Iterator;
import java.util.Set;

// Read-only observations of recoverable candidates. No journal is prepared or
// executed by this observer, and no Microsoft Access save/reopen is performed.
public final class ReadManualTables {
    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 3, "expected path, keyless/int32/int64/currency and 0/1 rows");
        String kind = args[1];
        int rowCount = Integer.parseInt(args[2]);
        require(rowCount == 0 || rowCount == 1, "candidate row count");
        DataType type = switch (kind) {
            case "keyless", "int32" -> DataType.LONG;
            case "int64" -> DataType.BIG_INT;
            case "currency" -> DataType.MONEY;
            default -> throw new IllegalArgumentException("unknown candidate kind");
        };
        boolean keyed = !kind.equals("keyless");
        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true).open()) {
            require(database.getTableNames().equals(Set.of("Manual")), "complete user table inventory");
            Table table = database.getTable("Manual");
            require(table.getColumnCount() == 2, "complete field count");
            Column first = table.getColumn("First");
            Column actualKey = table.getColumn("ActualKey");
            require(first.getColumnIndex() == 0 && first.getType() == DataType.LONG, "First field identity/type");
            require(actualKey.getColumnIndex() == 1 && actualKey.getType() == type, "ActualKey field identity/type");
            require(!first.isAutoNumber() && !actualKey.isAutoNumber(), "no invented AutoNumber");
            require(table.getIndexes().size() == (keyed ? 1 : 0), "complete index inventory");
            int primaryCount = 0;
            for (Index index : table.getIndexes()) {
                require(index.isPrimaryKey(), "only authored primary index");
                require(index.getName().equals("ActualPrimary"), "exact primary name");
                require(index.getColumns().size() == 1, "single primary field");
                require(index.getColumns().get(0).getName().equals("ActualKey"), "primary on actual field one");
                primaryCount++;
            }
            require(primaryCount == (keyed ? 1 : 0), "explicit primary absence/presence");
            require(table.getRowCount() == rowCount, "declared row count");
            Iterator<Row> rows = table.iterator();
            if (rowCount == 1) {
                require(rows.hasNext(), "inserted row exists");
                Row row = rows.next();
                require(Integer.valueOf(17).equals(row.get("First")), "exact supplied First Int32");
                Object value = row.get("ActualKey");
                switch (kind) {
                    case "keyless", "int32" -> require(Integer.valueOf(7).equals(value), "exact supplied Int32 key");
                    case "int64" -> require(Long.valueOf(7).equals(value), "exact supplied Int64 key");
                    case "currency" -> require(value instanceof BigDecimal
                        && ((BigDecimal) value).compareTo(new BigDecimal("7.5000")) == 0, "exact supplied Currency");
                    default -> throw new AssertionError("validated kind");
                }
            }
            require(!rows.hasNext(), "complete row scan/EOF");
            System.out.println(database.getFileFormat() + " Manual " + kind
                + " key-position=" + (keyed ? "1" : "absent") + " rows=" + rowCount);
        }
    }
}
