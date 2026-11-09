import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.io.File;
import java.util.HashSet;
import java.util.Set;
import java.util.TreeSet;

public final class ReadRepeatedAddition {
    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    private static Row originalRow(Database database, String name, String location) throws Exception {
        Table table = database.getTable(name);
        require(table.getRowCount() == 1, name + " row count");
        require(table.getPrimaryKeyIndex() != null, name + " primary index");
        Row row = table.iterator().next();
        require(row.getInt("Id") == 1, name + " Id");
        require(row.getId().toString().equals(location), name + " physical row identity");
        return row;
    }

    public static void main(String[] args) throws Exception {
        int count = Integer.parseInt(args[1]);
        require(count >= 2 && count <= 19, "fixture table count");
        Set<String> expected = new HashSet<>(Set.of("Items", "More"));
        for (int number = 3; number <= count; number++) {
            expected.add(number == 3 ? "Third" : "Table" + number);
        }

        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true).open()) {
            require(database.getTableNames().equals(expected), "complete user table inventory");
            Row items = originalRow(database, "Items", "8:0");
            Row more = originalRow(database, "More", "11:0");
            require(items.getInt("Score") == 17, "retained original Score");
            require(more.getString("Name").equals("Ada"), "retained second-table Name");

            for (String name : expected) {
                if (name.equals("Items") || name.equals("More")) {
                    continue;
                }
                Table table = database.getTable(name);
                require(table.getRowCount() == 0, name + " empty row count");
                require(table.getColumnCount() == 2, name + " field count");
                require(table.getColumn("Id").getType() == DataType.LONG, name + " Id type");
                require(table.getColumn("Id").isAutoNumber(), name + " AutoNumber marker");
                require(table.getColumn("Name").getType() == DataType.TEXT, name + " Name type");
                require(table.getColumn("Name").getLengthInUnits() == 20, name + " Text capacity");
                require(table.getPrimaryKeyIndex() != null, name + " primary index");
            }
            System.out.println(database.getFileFormat() + " " + new TreeSet<>(expected)
                + " original=" + items + " second=" + more);
        }
    }
}
