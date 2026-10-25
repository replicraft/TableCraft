import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;

/** Original inert fixture: ordinary table builders plus explicit system-query fragment rows. */
public final class ExternalNamedFixture {
    private Table catalog;
    private Table fragments;
    private int parent;
    private int nextId = Integer.MIN_VALUE + 24_000;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No external target may be resolved");
        });
    }

    private void row(int id, int attribute, String expression, Short flag,
                     String name1, String name2, int order) throws Exception {
        Map<String, Object> fields = new HashMap<>();
        fields.put("ObjectId", id);
        fields.put("Attribute", (byte) attribute);
        fields.put("Expression", expression);
        fields.put("Flag", flag);
        fields.put("Name1", name1);
        fields.put("Name2", name2);
        fields.put("Order", ByteBuffer.allocate(Integer.BYTES).putInt(order).array());
        fragments.addRowFromMap(fields);
    }

    private int query(String name, int family, int type) throws Exception {
        int id = nextId++;
        Map<String, Object> object = new HashMap<>();
        object.put("Id", id);
        object.put("ParentId", parent);
        object.put("Name", name);
        object.put("Type", (short) 5);
        object.put("Flags", family);
        catalog.addRowFromMap(object);
        row(id, 1, null, (short) type, null, null, 1);
        row(id, 3, null, (short) 0, null, null, 1);
        return id;
    }

    private void remote(int id, String path, String connection) throws Exception {
        row(id, 4, connection, (short) 2, path, null, 1);
    }

    private int select(String name, String source, String[][] fields) throws Exception {
        int id = query(name, 0, 1);
        row(id, 5, null, null, source, null, 1);
        int order = 1;
        for (String[] field : fields) {
            row(id, 6, field[0], null, field[1], null, order++);
        }
        return id;
    }

    private void cross(String name, boolean fixed) throws Exception {
        int id = query(name, 0x10, 6);
        row(id, 5, null, null, "Items", null, 1);
        row(id, 6, "7", (short) 2, "G", null, 1);
        row(id, 6, "Sum(Score)", (short) 0, "Total", null, 2);
        row(id, 6, fixed ? "Quarter IN (1,2)" : "Quarter", (short) 1, null, null, 3);
        row(id, 9, "7", (short) 2, null, null, 1);
    }

    private void write(Path path, String format, int node) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(format)).create()) {
            inert(database);
            database.setAllowAutoNumberInsert(true);
            Table items = new TableBuilder("Items")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addColumn(new ColumnBuilder("Quarter", DataType.LONG))
                .addColumn(new ColumnBuilder("Formula", DataType.TEXT).setLengthInUnits(100)).toTable(database);
            int count = node + 2;
            int first = node == 0 ? 1001 : node * 100 + 1;
            for (int position = 0; position < count; position++) {
                int score = node == 0 ? 100 + position * 10 : node == 1 ? (position + 1) * 10 : position + 1;
                int quarter = node == 0 ? 9 : node == 1 ? position % 2 + 1 : position % 2 + 2;
                items.addRow(first + position, score, quarter, (first + position) + "+100");
            }
            database.createLinkedTable("LinkedItems", "inert-never-opened", "Items");

            catalog = database.getSystemTable("MSysObjects");
            fragments = database.getSystemTable("MSysQueries");
            for (Row object : catalog) {
                if ("Tables".equals(object.get("Name"))
                        && Short.valueOf((short) 3).equals(object.get("Type"))) {
                    parent = object.getInt("Id");
                }
            }
            if (parent == 0) {
                throw new AssertionError("Missing catalog namespace");
            }

            select("Same", "Items", new String[][] {
                {"ID", "ID"}, {"Score", "Score"}, {"Quarter", "Quarter"}, {"Formula", "Formula"},
            });
            select("Nested", "Same", new String[][] {{"ID", "ID"}, {"Score", "Score"}});
            select("Owned", "Items", new String[][] {
                {"ID", "ID"}, {"(SELECT Count(*) FROM Items)", "LocalCount"},
                {"DCount('*','Items')", "BaseCount"}, {"DCount('*','Same')", "NamedCount"},
                {"Eval(Formula)", "Evaluated"},
            });
            int id = query("UnionRows", 0x80, 9);
            row(id, 5, "SELECT ID,Score FROM Items", null, null, "X7YZ_____1", 1);
            row(id, 5, "SELECT ID,Score FROM Same", null, null, "X7YZ_____2", 2);
            cross("FixedCross", true);
            cross("DynamicCross", false);

            id = select("Param", "Items", new String[][] {{"ID", "ID"}, {"Score+Floor", "Shifted"}});
            row(id, 2, null, (short) 4, "Floor", null, 1);
            id = select("Redirect", "Nested", new String[][] {{"ID", "ID"}, {"Score", "Score"}});
            remote(id, "third-token", null);
            select("BadEval", "Items", new String[][] {{"Eval('ID')", "V"}});
            select("DomainEval", "Items", new String[][] {{"Eval('DCount(''*'',''Items'')')", "V"}});
            select("LateError", "Items", new String[][] {
                {"ID", "ID"}, {"1/(ID-" + (first + count - 1) + ")", "V"},
            });
            select("Cycle", "Cycle", new String[][] {{"*", null}});
            select("Undeclared", "Items", new String[][] {{"Floor", "V"}});
            select("ParamDomain", "Items", new String[][] {{"DCount('*','Param')", "N"}});
            id = select("MissingRemote", "Items", new String[][] {{"ID", "ID"}});
            remote(id, "not-authorized", null);
            id = select("ProviderRemote", "Items", new String[][] {{"ID", "ID"}});
            remote(id, "third-token", "ODBC;");
            database.flush();
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh DIRECTORY FORMAT");
        }
        Path directory = Path.of(args[0]);
        Files.createDirectories(directory);
        String suffix = args[1].equals("V2000") ? ".mdb" : ".accdb";
        String[] names = {"root", "one", "two"};
        for (int node = 0; node < names.length; node++) {
            new ExternalNamedFixture().write(directory.resolve(names[node] + suffix), args[1], node);
        }
    }
}
