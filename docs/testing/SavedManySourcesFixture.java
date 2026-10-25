import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.query.QueryImpl;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Objects;

/** Original inert query fragments and read-only getters; never executes SQL. */
public final class SavedManySourcesFixture {
    private static Table catalog;
    private static Table fragments;
    private static int parent;
    private static int nextId = Integer.MIN_VALUE + 1200;

    private static void fragment(int id, int attribute, String expression, Short flag,
            String first, String second, int ordinal) throws Exception {
        byte[] order = ByteBuffer.allocate(Integer.BYTES).putInt(ordinal).array();
        fragments.addRowFromMap(new QueryImpl.Row(null, (byte) attribute, expression,
            flag, null, first, second, id, order).toTableRow());
    }

    private static void query(String name, int count, int joins, boolean crosstab)
            throws Exception {
        int id = nextId++;
        HashMap<String, Object> row = new HashMap<>();
        row.put("Id", id);
        row.put("ParentId", parent);
        row.put("Name", name);
        row.put("Type", (short) 5);
        row.put("Flags", crosstab ? 0x10 : 0);
        catalog.addRowFromMap(row);
        fragment(id, 1, null, (short) (crosstab ? 6 : 1), null, null, 1);
        for (int index = 0; index < count; index++) {
            fragment(id, 5, null, null, "Items", "a" + index, index + 1);
        }
        for (int index = 0; index < joins; index++) {
            // The seventeenth fragment is an explicit out-of-profile control.
            int pair = index % 16;
            String left = "a" + pair;
            String right = "a" + (31 - pair);
            fragment(id, 7, "[" + left + "].[ID]=[" + right + "].[ID]",
                (short) 1, left, right, index + 1);
        }
        if (crosstab) {
            fragment(id, 6, "0", (short) 2, "Bucket", null, 1);
            fragment(id, 6, "Count(*)", (short) 0, "Total", null, 2);
            fragment(id, 6, "a31.ID IN(7)", (short) 1, null, null, 3);
            fragment(id, 9, "0", (short) 2, null, null, 1);
            fragment(id, 9, "a31.ID", (short) 1, null, null, 2);
        } else {
            fragment(id, 6, "[a" + (count - 1) + "].[ID]", null, "Chosen", null, 1);
        }
    }

    private static void write(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            database.setEvaluateExpressions(false);
            database.setAllowAutoNumberInsert(true);
            new TableBuilder("Items").addColumn(new ColumnBuilder("ID", DataType.LONG))
                .toTable(database).addRow(7);
            catalog = database.getSystemTable("MSysObjects");
            fragments = database.getSystemTable("MSysQueries");
            for (Row row : catalog) {
                if ("Tables".equals(row.get("Name"))
                        && Objects.equals(row.get("Type"), (short) 3)) {
                    parent = row.getInt("Id");
                }
            }
            if (parent == 0) {
                throw new AssertionError("Missing query namespace");
            }
            query("Sources17", 17, 0, false);
            query("Sources32", 32, 0, false);
            query("Sources33", 33, 0, false);
            query("Mixed32", 32, 16, false);
            query("Joins17", 32, 17, false);
            query("Cross32", 32, 0, true);
            query("Cross33", 33, 0, true);
        }
    }

    private static void read(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked opens");
            });
            for (Row row : database.getTable("Items")) {
                System.out.println("ITEM\t" + row.getInt("ID"));
            }
            Table stored = database.getSystemTable("MSysQueries");
            for (Query query : database.getQueries()) {
                int tables = 0;
                int joins = 0;
                for (Row row : stored) {
                    if (!Objects.equals(row.get("ObjectId"), query.getObjectId())) {
                        continue;
                    }
                    Byte attribute = row.getByte("Attribute");
                    tables += attribute != null && attribute == 5 ? 1 : 0;
                    joins += attribute != null && attribute == 7 ? 1 : 0;
                }
                System.out.println("QUERY\t" + query.getName() + "\t" + query.getType()
                    + "\t" + tables + "\t" + joins);
                System.out.println("SQL\t" + query.getName() + "\t"
                    + query.toSQLString().replace("\r", "").replace("\n", " "));
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("write")) {
            write(Path.of(args[1]), args[2]);
        } else if (args.length == 2 && args[0].equals("read")) {
            read(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected write PATH FORMAT or read PATH");
        }
    }
}
