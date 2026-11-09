import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.io.File;
import java.math.BigDecimal;
import java.util.HashSet;
import java.util.Iterator;
import java.util.Set;

// Observe the independent five-table source and native append/insert candidates.
// No write, journal operation or native Access execution occurs here.
public final class ReadMixedTables {
    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    private static DataType keyType(String kind) {
        return switch (kind) {
            case "keyless", "int32", "generated" -> DataType.LONG;
            case "int64" -> DataType.BIG_INT;
            case "currency" -> DataType.MONEY;
            default -> throw new IllegalArgumentException("unknown key family");
        };
    }

    private static void schema(Table table, String kind, boolean actualSecond) {
        String keyName = actualSecond ? "ActualKey" : "Id";
        require(table.getColumnCount() == 2, "complete column inventory");
        Column key = table.getColumn(keyName);
        Column score = table.getColumn("Score");
        require(key.getColumnIndex() == (actualSecond ? 1 : 0), "actual key position");
        require(score.getColumnIndex() == (actualSecond ? 0 : 1), "actual Score position");
        require(key.getType() == keyType(kind), "key native type");
        require(score.getType() == DataType.LONG && !score.isAutoNumber(), "ordinary Score type");
        require(key.isAutoNumber() == kind.equals("generated"), "exact generation flag");
        boolean keyed = !kind.equals("keyless");
        require(table.getIndexes().size() == (keyed ? 1 : 0), "complete index inventory");
        for (Index index : table.getIndexes()) {
            require(index.isPrimaryKey(), "only a primary index");
            require(index.getName().equals(actualSecond ? "ActualPrimary" : "PrimaryId"), "primary name");
            require(index.getColumns().size() == 1, "one indexed field");
            require(index.getColumns().get(0).getName().equals(keyName), "actual indexed field");
        }
    }

    private static void empty(Table table) {
        require(table.getRowCount() == 0 && !table.iterator().hasNext(), "empty table and EOF");
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 3, "expected path, source/key-family and 0/1 rows");
        String kind = args[1];
        int count = Integer.parseInt(args[2]);
        require(count == 0 || count == 1, "candidate row count");
        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true).open()) {
            Set<String> expected = new HashSet<>(Set.of("Items", "Mixed2", "Mixed3", "Mixed4", "Mixed5"));
            if (!kind.equals("source")) {
                expected.add("Added");
            }
            require(database.getTableNames().equals(expected), "complete user-table inventory");
            Table items = database.getTable("Items");
            schema(items, "generated", false);
            require(items.getRowCount() == 1, "retained original row count");
            Iterator<Row> original = items.iterator();
            require(original.hasNext(), "original row exists");
            Row retained = original.next();
            require(Integer.valueOf(1).equals(retained.get("Id")), "retained original Id");
            require(Integer.valueOf(10).equals(retained.get("Score")), "retained original Score");
            require(!original.hasNext(), "complete original EOF");

            String[] sourceKinds = {"keyless", "int32", "int64", "currency"};
            for (int index = 0; index < sourceKinds.length; index++) {
                Table table = database.getTable("Mixed" + (index + 2));
                schema(table, sourceKinds[index], false);
                empty(table);
            }
            if (!kind.equals("source")) {
                Table added = database.getTable("Added");
                boolean generated = kind.equals("generated");
                schema(added, kind, !generated);
                require(added.getRowCount() == count, "added declared row count");
                Iterator<Row> rows = added.iterator();
                if (count == 1) {
                    require(rows.hasNext(), "new row exists");
                    Row row = rows.next();
                    require(Integer.valueOf(17).equals(row.get("Score")), "supplied Score");
                    Object key = row.get(generated ? "Id" : "ActualKey");
                    switch (kind) {
                        case "generated" -> require(Integer.valueOf(1).equals(key), "generated Id");
                        case "keyless", "int32" -> require(Integer.valueOf(7).equals(key), "supplied Int32");
                        case "int64" -> require(Long.valueOf(7).equals(key), "supplied Int64");
                        case "currency" -> require(key instanceof BigDecimal
                            && ((BigDecimal) key).compareTo(new BigDecimal("7.5000")) == 0, "supplied Currency");
                        default -> throw new AssertionError("validated key family");
                    }
                }
                require(!rows.hasNext(), "complete new-table EOF");
            }
            System.out.println(database.getFileFormat() + " mixed-source " + kind + " rows=" + count);
        }
    }
}
