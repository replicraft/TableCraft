import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.query.QueryImpl;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;
import java.security.MessageDigest;
import java.util.Arrays;

/** Original inert fixture recipe; only explicit native metadata rows are changed. */
class ConstantQuerySaveFixture {
    private static final int QUERY_ID = Integer.MIN_VALUE + 7000;

    private static byte[] order() {
        return ByteBuffer.allocate(4).putInt(1).array();
    }

    private static void fragment(Table table, byte attribute, String expression,
                                 Short flag, String name) throws Exception {
        table.addRowFromMap(new QueryImpl.Row(null, attribute, expression, flag,
                null, name, null, QUERY_ID, order()).toTableRow());
    }

    private static void observe(Path path, String expected) throws Exception {
        try (Database db = new DatabaseBuilder(path).setReadOnly(true).open()) {
            db.setEvaluateExpressions(false);
            Query query = db.getQueries().stream()
                    .filter(candidate -> candidate.getName().equals("Constants"))
                    .findFirst().orElseThrow();
            if (query.getObjectId() != QUERY_ID || query.getType() != Query.Type.SELECT) {
                throw new AssertionError("query identity changed");
            }
            if (!query.toSQLString().contains(expected + " AS Answer")) {
                throw new AssertionError("unexpected query rendering");
            }
            System.out.println(path.getFileName() + "\t" + query.getObjectId()
                    + "\t" + query.toSQLString().replace("\n", "\\n"));
        }
    }

    public static void main(String[] args) throws Exception {
        Path before = Path.of(args[0]);
        Path after = Path.of(args[1]);
        if (Files.exists(before) || Files.exists(after)) {
            throw new IllegalArgumentException("fresh paths required");
        }
        try (Database db = new DatabaseBuilder(before)
                .setFileFormat(Database.FileFormat.V2016).create()) {
            db.setEvaluateExpressions(false);
            db.setAllowAutoNumberInsert(true);
            Table catalog = db.getSystemTable("MSysObjects");
            int parent = 0;
            for (Row row : catalog) {
                if ("Tables".equals(row.get("Name"))
                        && Short.valueOf((short) 3).equals(row.get("Type"))) {
                    parent = row.getInt("Id");
                }
            }
            if (parent == 0) {
                throw new AssertionError("missing Tables namespace");
            }
            Map<String, Object> object = new HashMap<>();
            object.put("Id", QUERY_ID);
            object.put("ParentId", parent);
            object.put("Name", "Constants");
            object.put("Type", (short) 5);
            object.put("Flags", 0);
            catalog.addRowFromMap(object);
            Table queries = db.getSystemTable("MSysQueries");
            fragment(queries, (byte) 0, null, null, null);
            fragment(queries, (byte) 1, null, (short) 1, null);
            fragment(queries, (byte) 3, null, (short) 0, null);
            fragment(queries, (byte) 6, "42", null, "Answer");
            fragment(queries, (byte) 255, null, null, null);
            db.flush();
        }
        observe(before, "42");
        byte[] originalHash = MessageDigest.getInstance("SHA-256")
                .digest(Files.readAllBytes(before));
        Files.copy(before, after);
        try (Database db = new DatabaseBuilder(after).open()) {
            db.setEvaluateExpressions(false);
            Table queries = db.getSystemTable("MSysQueries");
            int changed = 0;
            for (Row row : queries) {
                if (Integer.valueOf(QUERY_ID).equals(row.get("ObjectId"))
                        && Byte.valueOf((byte) 6).equals(row.get("Attribute"))) {
                    row.put("Expression", "43");
                    queries.updateRow(row);
                    changed++;
                }
            }
            if (changed != 1) {
                throw new AssertionError("expected one projection row");
            }
            db.flush();
        }
        observe(before, "42");
        observe(after, "43");
        byte[] finalHash = MessageDigest.getInstance("SHA-256")
                .digest(Files.readAllBytes(before));
        if (!Arrays.equals(originalHash, finalHash)) {
            throw new AssertionError("original fixture changed");
        }
    }
}
