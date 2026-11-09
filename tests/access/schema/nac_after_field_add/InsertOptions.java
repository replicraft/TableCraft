import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.PropertyMapImpl;
import java.io.File;
import java.util.HashMap;
import java.util.Map;
import java.util.HexFormat;

// Explicit engineering write on a fresh copy, never an original corpus input.
public final class InsertOptions {
    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    private static void observe(Database database) throws Exception {
        int roles = 0;
        for (Row row : database.getSystemTable("MSysObjects")) {
            if ("MSysDb".equals(row.getString("Name"))) {
                roles++;
                System.out.printf("role\t%d\t%d\t%d\t%d%n", row.getInt("Id"),
                        row.getInt("ParentId"), row.getShort("Type"), row.getInt("Flags"));
            }
        }
        System.out.printf("roles\t%d%n", roles);
        for (PropertyMap.Property property : database.getDatabaseProperties()) {
            System.out.printf("property\t%s\t%s\t%s\t%s%n", property.getName(),
                    property.getType(), property.isDdl(), property.getValue());
        }
        for (String name : database.getTableNames()) {
            Table table = database.getTable(name);
            System.out.printf("table\t%s\t%d\t%d\t%d%n", name, table.getColumnCount(),
                    table.getRowCount(), table.getIndexes().size());
        }
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 2, "expected mode and fresh-copy path");
        File file = new File(args[1]);
        if ("read".equals(args[0])) {
            try (Database database = new DatabaseBuilder(file).setReadOnly(true).open()) {
                database.setEvaluateExpressions(false);
                observe(database);
            }
            return;
        }
        if ("missing-role-save".equals(args[0])) {
            try (Database database = new DatabaseBuilder(file).open()) {
                database.setEvaluateExpressions(false);
                PropertyMap properties = database.getDatabaseProperties();
                require(properties.isEmpty(), "expected absent database properties");
                properties.put("Track Name AutoCorrect Info", DataType.LONG, 0, true);
                properties.put("Perform Name AutoCorrect", DataType.LONG, 0, true);
                properties.save();
            }
            return;
        }
        require("insert-off".equals(args[0]) || "insert-complete-off".equals(args[0]), "unsupported mode");
        try (Database database = new DatabaseBuilder(file).open()) {
            database.setEvaluateExpressions(false);
            Table catalog = database.getSystemTable("MSysObjects");
            for (Row row : catalog) {
                require(!"MSysDb".equals(row.getString("Name")), "MSysDb already exists");
            }
            Map<String, Object> row = new HashMap<>();
            row.put("Id", 0x0f000004);
            row.put("ParentId", 0x0f000002);
            row.put("Name", "MSysDb");
            row.put("Type", (short) 2);
            row.put("Flags", 0x80000000);
            if ("insert-complete-off".equals(args[0])) {
                PropertyMap properties = database.getDatabaseProperties();
                properties.put("Track Name AutoCorrect Info", DataType.LONG, 0, true);
                properties.put("Perform Name AutoCorrect", DataType.LONG, 0, true);
                byte[] bytes = ((PropertyMapImpl) properties).getOwner().write();
                System.out.printf("serialized-properties\t%d\t%s%n", bytes.length,
                        HexFormat.of().formatHex(bytes));
                row.put("LvProp", bytes);
            }
            catalog.addRowFromMap(row);
        }
        if ("insert-complete-off".equals(args[0])) {
            try (Database database = new DatabaseBuilder(file).setReadOnly(true).open()) {
                database.setEvaluateExpressions(false);
                observe(database);
            }
            return;
        }
        // The missing-role getter caches a row-less map, so reopen the new role.
        try (Database database = new DatabaseBuilder(file).open()) {
            database.setEvaluateExpressions(false);
            PropertyMap properties = database.getDatabaseProperties();
            require(properties.isEmpty(), "expected an empty new default map");
            properties.put("Track Name AutoCorrect Info", DataType.LONG, 0, true);
            properties.put("Perform Name AutoCorrect", DataType.LONG, 0, true);
            properties.save();
        }
        try (Database database = new DatabaseBuilder(file).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            observe(database);
        }
    }
}
