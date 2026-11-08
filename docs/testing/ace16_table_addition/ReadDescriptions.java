import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Table;
import java.io.File;
import java.util.HashSet;
import java.util.Set;
import java.util.TreeSet;

// Read-only observation of TableCraft-produced description candidates, not native
// Access save/reopen evidence. Descriptive Now() text must remain inert metadata.
public final class ReadDescriptions {
    private static final String EXACT = "  Résumé 日本語 Now()  ";

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    private static Column field(Table table, String name, DataType type, boolean autoNumber)
            throws Exception {
        Column column = table.getColumn(name);
        require(column.getType() == type, table.getName() + "." + name + " type");
        require(column.isAutoNumber() == autoNumber, table.getName() + "." + name + " AutoNumber");
        return column;
    }

    private static void description(Column column, String expected) throws Exception {
        PropertyMap.Property property = column.getProperties().get("Description");
        if (expected == null) {
            require(property == null, column.getName() + " absent Description");
        } else {
            require(property != null, column.getName() + " present Description");
            require(property.getType() == DataType.TEXT, column.getName() + " Description carrier");
            require(property.isDdl(), column.getName() + " Description declaration flag");
            require(expected.equals(property.getValue()), column.getName() + " exact Description");
        }
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 2, "expected candidate path and table count");
        int count = Integer.parseInt(args[1]);
        require(count >= 1 && count <= 3, "candidate table count");
        Set<String> names = new HashSet<>(Set.of("Properties"));
        if (count >= 2) {
            names.add("Second");
        }
        if (count == 3) {
            names.add("Third");
        }
        String boundary = "é".repeat(253) + "😀";
        require(boundary.length() == 255, "independent UTF-16 boundary length");

        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true).open()) {
            require(database.getTableNames().equals(names), "complete user table inventory");
            for (String name : names) {
                Table table = database.getTable(name);
                require(table.getRowCount() == 0, name + " empty row count");
                require(!table.iterator().hasNext(), name + " complete empty scan");
                require(table.getPrimaryKeyIndex() != null, name + " primary index");
                require(table.getColumnCount() == (name.equals("Properties") ? 4 : 2), name + " field count");
                Column id = field(table, "Id", DataType.LONG, true);
                Column label = field(table, "Label", DataType.TEXT, false);
                require(label.getLengthInUnits() == 255, name + " Text capacity");
                description(id, name.equals("Properties") ? boundary : null);
                description(label, EXACT);
                if (name.equals("Properties")) {
                    description(field(table, "Empty", DataType.LONG, false), "");
                    description(field(table, "Flag", DataType.BOOLEAN, false), null);
                }
            }
            System.out.println(database.getFileFormat() + " " + new TreeSet<>(names)
                + " descriptions=exact empty=present absent=absent rows=0");
        }
    }
}
