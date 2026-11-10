import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

// First-party read-only qualification of independently constructed fixture bytes.
public final class ReadMixed {
    static void require(boolean value, String message) {
        if (!value) {
            throw new IllegalStateException(message);
        }
    }

    static void equal(Object actual, Object expected, String message) {
        if (actual instanceof byte[] left && expected instanceof byte[] right) {
            require(Arrays.equals(left, right), message);
        } else {
            require(Objects.equals(actual, expected), message + ": " + actual);
        }
    }

    public static void main(String[] paths) throws Exception {
        for (String argument : paths) {
            Path path = Path.of(argument);
            String name = path.getFileName().toString();
            boolean binary = name.startsWith("binary-");
            boolean later = name.startsWith("later-");
            int selected = name.contains("-after-")
                ? Integer.parseInt(name.substring(name.length() - 7, name.length() - 6))
                : -1;
            byte[] original = Files.readAllBytes(path);
            try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
                database.setEvaluateExpressions(false);
                Table table = database.getTable("Table1");
                require(table.getColumnCount() == (later ? 6 : 5), "column count");
                require(table.getRowCount() == 3, "logical row count");
                Column bonus = table.getColumn("Bonus");
                equal(bonus.getType(), DataType.LONG, "new Number type");
                equal(bonus.getProperties().getValue("DefaultValue"), "0", "stored default");
                equal(bonus.getProperties().getValue("Required"), false, "stored required");
                equal(table.getColumn("OtherLabel").getType(),
                    binary ? DataType.BINARY : DataType.TEXT, "variable type");
                Set<Integer> ids = new TreeSet<>();
                for (Row row : table) {
                    int id = row.getInt("ID");
                    require(ids.add(id), "duplicate logical ID");
                    equal(row.get("Label"),
                        id == 1 ? "alpha" : id == 2 ? null : "wide", "old text");
                    equal(row.get("AfterLabel"),
                        id == 1 ? Integer.valueOf(42) : id == 2 ? null : Integer.valueOf(-7),
                        "old number");
                    Object other;
                    if (id == 3) {
                        other = null;
                    } else if (binary) {
                        other = id == 1 ? new byte[]{0, (byte) 255, 1, 2} : new byte[0];
                    } else {
                        other = id == 1 ? "\u2603" : "";
                    }
                    equal(row.get("OtherLabel"), other, "other variable");
                    equal(row.get("Bonus"),
                        id == selected + 1 ? Integer.valueOf(37) : null, "new Number");
                    if (later) {
                        equal(row.get("FutureLabel"), null, "other absent variable remains NULL");
                    }
                }
                equal(ids, new TreeSet<>(Arrays.asList(1, 2, 3)), "logical IDs");
                IndexCursor cursor = table.getPrimaryKeyIndex().newCursor().toIndexCursor();
                int indexed = 0;
                for (Row row : cursor) {
                    indexed++;
                    equal(row.get("ID"), indexed, "original index root reference");
                    equal(row.get("Bonus"),
                        indexed == selected + 1 ? Integer.valueOf(37) : null,
                        "index follows grown row");
                }
                require(indexed == 3, "index membership");
            }
            require(Arrays.equals(original, Files.readAllBytes(path)),
                "read-only observation changed bytes");
            System.out.println(name + "\trows=3\tindexed=3\tunchanged=true");
        }
    }
}
