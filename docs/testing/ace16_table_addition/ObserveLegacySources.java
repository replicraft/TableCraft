import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.Table;
import java.io.File;
import java.util.Set;

// Private read-only qualification of synthetic retained source declarations.
public final class ObserveLegacySources {
    private static void require(boolean value, String message) {
        if (!value) {
            throw new AssertionError(message);
        }
    }

    private static void empty(Table table) throws Exception {
        require(table.getRowCount() == 0, table.getName() + " declared empty");
        require(!table.iterator().hasNext(), table.getName() + " actual EOF");
    }

    private static void primary(Table table, String name, String field, boolean present) throws Exception {
        require(table.getIndexes().size() == (present ? 1 : 0), table.getName() + " index inventory");
        for (Index index : table.getIndexes()) {
            require(index.isPrimaryKey(), table.getName() + " primary marker");
            require(index.getName().equals(name), table.getName() + " primary name");
            require(index.getColumns().size() == 1, table.getName() + " primary arity");
            require(index.getColumns().get(0).getName().equals(field), table.getName() + " primary field");
        }
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 3, "expected path, memo/ole and source/keyless/keyed");
        DataType kind = switch (args[1]) {
            case "memo" -> DataType.MEMO;
            case "ole" -> DataType.OLE;
            default -> throw new IllegalArgumentException("unknown carrier");
        };
        String mode = args[2];
        require(Set.of("source", "keyless", "keyed").contains(mode), "candidate mode");
        boolean added = !mode.equals("source");
        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true).open()) {
            require(database.getTableNames().equals(added ? Set.of("Items", "Manual") : Set.of("Items")), "complete table inventory");
            Table table = database.getTable("Items");
            require(table.getColumnCount() == 2, "Items field inventory");
            Column id = table.getColumn("Id");
            require(id.getColumnIndex() == 0 && id.getType() == DataType.LONG && id.isAutoNumber(), "original LONG AutoNumber Id");
            Column score = table.getColumn("Score");
            require(score.getColumnIndex() == 1 && score.getType() == kind && !score.isAutoNumber(), "original Long Text/OLE Score");
            primary(table, "PrimaryId", "Id", true);
            empty(table);
            if (added) {
                Table manual = database.getTable("Manual");
                require(manual.getColumnCount() == 2, "Manual field inventory");
                for (String name : new String[]{"First", "ActualKey"}) {
                    Column column = manual.getColumn(name);
                    require(column.getType() == DataType.LONG && !column.isAutoNumber(), "ordinary manual field " + name);
                }
                require(manual.getColumn("First").getColumnIndex() == 0 && manual.getColumn("ActualKey").getColumnIndex() == 1, "manual field order");
                primary(manual, "ActualPrimary", "ActualKey", mode.equals("keyed"));
                empty(manual);
            }
            System.out.println(database.getFileFormat() + " " + args[1] + " " + mode + " schema=exact rows=0");
        }
    }
}
