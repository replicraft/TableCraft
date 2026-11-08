import com.healthmarketscience.jackcess.*;
import java.io.File;
import java.util.Set;

public final class ReadAddition {
    private static void require(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        boolean added = Boolean.parseBoolean(args[1]);
        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true).open()) {
            require(database.getTableNames().equals(added ? Set.of("Items", "More") : Set.of("Items")), "user table inventory");
            Table original = database.getTable("Items");
            require(original.getRowCount() == 1, "original row count");
            Row row = original.iterator().next();
            require(((Number) row.get("Id")).intValue() == 1, "original Id");
            require(((Number) row.get("Score")).intValue() == 17, "original Score");
            require(original.getPrimaryKeyIndex() != null, "original primary index");
            if (added) {
                Table more = database.getTable("More");
                require(more.getRowCount() == 0, "empty appended table");
                require(more.getColumn("Id").getType() == DataType.LONG, "new Id carrier");
                require(more.getColumn("Id").isAutoNumber(), "new Increment flag");
                require(more.getColumn("Name").getType() == DataType.TEXT, "new Name carrier");
                require(more.getColumn("Name").getLengthInUnits() == 20, "new text capacity");
                require(more.getPrimaryKeyIndex() != null, "new primary index");
                require(!more.iterator().hasNext(), "no manufactured row");
            }
            System.out.println(database.getFileFormat() + " " + database.getTableNames() + " original=" + row);
        }
    }
}
