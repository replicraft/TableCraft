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
public final class SavedExternalFixture {
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

    private int select(String name, String path, String connection) throws Exception {
        int id = query(name, 0, 1);
        row(id, 5, null, null, "People", "P", 1);
        row(id, 6, "P.ID", null, "Person", null, 1);
        remote(id, path, connection);
        return id;
    }

    private void write(Path path, String format) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(format)).create()) {
            inert(database);
            database.setAllowAutoNumberInsert(true);
            Table people = new TableBuilder("People")
                .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(database);
            people.addRow(100);
            people.addRow(101);

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

            select("EmptyPath", "", null);
            select("PathOnly", "remote-token", null);
            select("WithConnection", "remote-token", "MDB_FILE;");
            select("EmptyConnection", "remote-token", "");
            select("NullPathConnection", null, "MDB_FILE;");
            select("LiteralPunctuation", "opaque ' é [x]; -- token", "TYPE]; ' marker");

            int id = select("JoinParameter", "remote-token", null);
            row(id, 2, null, (short) 4, "Floor", null, 1);
            row(id, 5, null, null, "Orders", "O", 2);
            row(id, 7, "P.ID=O.Person", (short) 1, "P", "O", 1);
            row(id, 8, "P.ID>=Floor", null, null, null, 1);

            id = query("CrossExternal", 0x10, 6);
            remote(id, "remote-token", null);
            row(id, 5, null, null, "Orders", "O", 1);
            row(id, 6, "O.Person", (short) 2, "Person", null, 1);
            row(id, 6, "Count(*)", (short) 0, "N", null, 2);
            row(id, 6, "O.Quarter IN (1,2)", (short) 1, null, null, 3);
            row(id, 9, "O.Person", (short) 2, null, null, 1);

            id = query("UnionExternal", 0x80, 9);
            row(id, 5, "SELECT ID AS Person FROM People IN 'remote-token'", null,
                null, "X7YZ_____1", 1);
            row(id, 5, "SELECT ID FROM People IN 'second-token'", null,
                null, "X7YZ_____2", 2);

            id = query("CrossDynamic", 0x10, 6);
            remote(id, "remote-token", null);
            row(id, 5, null, null, "Orders", "O", 1);
            row(id, 6, "O.Person", (short) 2, "Person", null, 1);
            row(id, 6, "Count(*)", (short) 0, "N", null, 2);
            row(id, 6, "O.Quarter", (short) 1, null, null, 3);
            row(id, 9, "O.Person", (short) 2, null, null, 1);

            id = select("LateError", "remote-token", null);
            row(id, 6, "10/(P.ID-3)", null, "Quotient", null, 2);

            id = select("RootDomain", "remote-token", null);
            row(id, 6, "DCount('*','People')", null, "RootCount", null, 2);

            database.flush();
        }
    }

    private static void writeBackend(Path path, String format) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(format)).create()) {
            inert(database);
            Table people = new TableBuilder("People")
                .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(database);
            for (int id = 1; id <= 3; id++) {
                people.addRow(id);
            }

            Table orders = new TableBuilder("Orders")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Person", DataType.LONG))
                .addColumn(new ColumnBuilder("Quarter", DataType.LONG)).toTable(database);
            orders.addRow(101, 1, 1);
            orders.addRow(102, 1, 2);
            orders.addRow(103, 2, 1);
            orders.addRow(104, 3, 2);

            database.flush();
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected OUTPUT_DIRECTORY FORMAT");
        }

        Path directory = Path.of(args[0]);
        Files.createDirectories(directory);
        String suffix = args[1].equals("V2000") ? ".mdb" : ".accdb";
        new SavedExternalFixture().write(directory.resolve("frontend" + suffix), args[1]);
        writeBackend(directory.resolve("backend" + suffix), args[1]);
    }
}
