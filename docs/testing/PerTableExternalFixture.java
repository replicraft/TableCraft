import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.query.BaseSelectQuery;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.HashMap;
import java.util.Map;

/** Original inert authoring/getter probe. No query or link target executes. */
public final class PerTableExternalFixture {
    private Table catalog;
    private Table fragments;
    private int parent;
    private int nextId = Integer.MIN_VALUE + 28_000;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("The probe must not resolve external paths");
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

    private int query(String name) throws Exception {
        int id = nextId++;
        Map<String, Object> object = new HashMap<>();
        object.put("Id", id);
        object.put("ParentId", parent);
        object.put("Name", name);
        object.put("Type", (short) 5);
        object.put("Flags", 0);
        catalog.addRowFromMap(object);
        row(id, 1, null, (short) 1, null, null, 1);
        row(id, 3, null, (short) 0, null, null, 1);
        return id;
    }

    private void remote(String name, String token, String alias) throws Exception {
        int id = query(name);
        row(id, 5, token, null, "Remote", alias, 1);
        row(id, 6, alias == null ? "[Remote].[ID]" : "[" + alias + "].[ID]",
            null, "Person", null, 1);
    }

    private void attach(Database database) throws Exception {
        catalog = database.getSystemTable("MSysObjects");
        fragments = database.getSystemTable("MSysQueries");
        for (Row object : catalog) {
            if ("Tables".equals(object.get("Name"))
                    && Short.valueOf((short) 3).equals(object.get("Type"))) {
                parent = object.getInt("Id");
            }
        }
        if (parent == 0) {
            throw new AssertionError("Missing native query namespace");
        }
    }

    private void root(Path path, Database.FileFormat format) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Refuse to overwrite an original");
        }
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            inert(database);
            database.setAllowAutoNumberInsert(true);
            Table local = new TableBuilder("Local")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Factor", DataType.LONG))
                .toTable(database);
            local.addRow(1, 10);
            local.addRow(3, 30);
            Table remote = new TableBuilder("Remote")
                .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(database);
            remote.addRow(999);
            attach(database);
            remote("RemoteOnly", "remote-token", null);
            remote("RemoteAlias", "remote-token", "r");
            remote("PunctuatedToken", "opaque ' é [x]; -- token", "r");
            remote("WindowsPath", "C:\\temp\\Database1.accdb", "r");
            remote("EmptyQualifier", "", "r");
            int wrapper = query("Wrapper");
            row(wrapper, 5, null, null, "RemoteOnly", null, 1);
            row(wrapper, 6, "[Person]", null, null, null, 1);
            int named = query("TargetNamed");
            row(named, 5, "remote-token", null, "TargetView", "q", 1);
            row(named, 6, "[q].[ID]", null, null, null, 1);
            row(named, 6, "[q].[Score]", null, null, null, 2);
            int combined = query("GlobalAndLeaf");
            row(combined, 4, null, (short) 2, "default-token", null, 1);
            row(combined, 5, "remote-token", null, "Remote", "r", 1);
            row(combined, 5, null, null, "Remote", "g", 2);
            row(combined, 6, "[r].[ID]", null, "RemoteID", null, 1);
            row(combined, 6, "[g].[ID]", null, "GlobalID", null, 2);
            row(combined, 7, "[r].[ID]=[g].[ID]", (short) 1, "r", "g", 1);
            row(combined, 11, "[r].[ID]", null, "A", null, 1);
            int id = query("MixedJoin");
            row(id, 5, null, null, "Local", "l", 1);
            row(id, 5, "remote-token", null, "Remote", "r", 2);
            row(id, 6, "[r].[ID]", null, "Person", null, 1);
            row(id, 6, "[l].[Factor]", null, "Factor", null, 2);
            row(id, 7, "[l].[ID]=[r].[ID]", (short) 1, "l", "r", 1);
            row(id, 11, "[r].[ID]", null, "A", null, 1);
            database.flush();
        }
    }

    private void target(Path path, Database.FileFormat format, int first) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Refuse to overwrite an original");
        }
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            inert(database);
            Table remote = new TableBuilder("Remote")
                .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(database);
            remote.addRow(first);
            remote.addRow(first + 1);
            remote.addRow(first + 2);
            database.setAllowAutoNumberInsert(true);
            attach(database);
            int id = query("TargetView");
            row(id, 5, null, null, "Remote", null, 1);
            row(id, 6, "[Remote].[ID]", null, null, null, 1);
            row(id, 6, "[Remote].[ID]*10", null, "Score", null, 2);
            database.flush();
        }
    }

    private static String encode(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof Number) {
            return "I" + value;
        }
        byte[] bytes = value instanceof byte[] raw ? raw
            : value.toString().getBytes(StandardCharsets.UTF_8);
        return (value instanceof byte[] ? "B" : "T")
            + Base64.getEncoder().encodeToString(bytes);
    }

    private static void emit(Object... values) {
        StringBuilder line = new StringBuilder();
        for (Object value : values) {
            if (line.length() > 0) {
                line.append('\t');
            }
            line.append(encode(value));
        }
        System.out.println(line);
    }

    private static void read(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (Query query : database.getQueries()) {
                emit("QUERY", query.getName(), query.getObjectId(), query.toSQLString());
                if (query instanceof BaseSelectQuery select) {
                    for (String source : select.getFromTables()) {
                        emit("FROM", query.getName(), source);
                    }
                    emit("GLOBAL", query.getName(), select.getFromRemoteDbPath(),
                        select.getFromRemoteDbType());
                }
            }
            for (Row row : database.getSystemTable("MSysQueries")) {
                emit("RAW", row.get("ObjectId"), row.get("Attribute"), row.get("Expression"),
                    row.get("Flag"), row.get("LvExtra"), row.get("Name1"), row.get("Name2"),
                    row.get("Order"));
            }
            for (String name : database.getTableNames()) {
                for (Row row : database.getTable(name)) {
                    emit("CELL", name, row.get("ID"), row.get("Factor"));
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected external handle activation");
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("write")) {
            Path directory = Path.of(arguments[1]);
            Database.FileFormat format = Database.FileFormat.valueOf(arguments[2]);
            String suffix = format == Database.FileFormat.V2000 ? ".mdb" : ".accdb";
            new PerTableExternalFixture().root(directory.resolve("root" + suffix), format);
            new PerTableExternalFixture().target(directory.resolve("target" + suffix), format, 1);
            new PerTableExternalFixture().target(directory.resolve("other" + suffix), format, 2);
        } else if (arguments.length == 2 && arguments[0].equals("read")) {
            read(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected write DIRECTORY FORMAT or read DATABASE");
        }
    }
}
