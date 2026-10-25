import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.query.QueryFormat;
import com.healthmarketscience.jackcess.impl.query.QueryImpl;
import com.healthmarketscience.jackcess.query.Query;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;

/** Original inert frontend/backend inputs; no linked SQL or expression executes. */
public class LinkedQueryFixture {
    private static String suffix(Database.FileFormat format) {
        return format == Database.FileFormat.V2000 ? ".mdb" : ".accdb";
    }

    private static void fragment(Table queries, int id, byte attribute, String expression,
            Short flag, String first, String second, int ordinal) throws Exception {
        byte[] order = ByteBuffer.allocate(4).putInt(ordinal).array();
        queries.addRowFromMap(new QueryImpl.Row(null, attribute, expression, flag,
                null, first, second, id, order).toTableRow());
    }

    private static void savedRoot(Database database) throws Exception {
        Table catalog = database.getSystemTable("MSysObjects");
        Table queries = database.getSystemTable("MSysQueries");
        int parent = 0;
        for (Row row : catalog) {
            if ("Tables".equals(row.get("Name")) && Objects.equals(row.get("Type"), (short) 3)) {
                parent = row.getInt("Id");
            }
        }
        if (parent == 0) {
            throw new IllegalStateException("Missing native query namespace");
        }
        int id = Integer.MIN_VALUE + 200;
        Map<String, Object> row = new HashMap<>();
        row.put("Id", id);
        row.put("ParentId", parent);
        row.put("Name", "Linked root");
        row.put("Type", (short) 5);
        row.put("Flags", 0);
        catalog.addRowFromMap(row);
        fragment(queries, id, QueryFormat.START_ATTRIBUTE, null, null, null, null, 1);
        fragment(queries, id, QueryFormat.TYPE_ATTRIBUTE, null,
                Query.Type.SELECT.getValue(), null, null, 1);
        fragment(queries, id, QueryFormat.FLAG_ATTRIBUTE, null, (short) 0, null, null, 1);
        fragment(queries, id, QueryFormat.TABLE_ATTRIBUTE, null, null, "People link", "p", 1);
        fragment(queries, id, QueryFormat.COLUMN_ATTRIBUTE, "[p].[ID]", null, "Person", null, 1);
        fragment(queries, id, QueryFormat.COLUMN_ATTRIBUTE, "[p].[Label]", null, "Label", null, 2);
        fragment(queries, id, QueryFormat.WHERE_ATTRIBUTE, "[p].[ID] >= 2", null, null, null, 1);
        fragment(queries, id, QueryFormat.ORDERBY_ATTRIBUTE, "[p].[ID]", null, "A", null, 1);
        fragment(queries, id, QueryFormat.END_ATTRIBUTE, null, null, null, null, 1);
    }

    private static void metadataCopy(String[] arguments) throws Exception {
        Path original = Path.of(arguments[1]);
        Path copy = Path.of(arguments[2]);
        Files.copy(original, copy);
        try (Database database = new DatabaseBuilder(copy).open()) {
            database.setEvaluateExpressions(false);
            database.setAllowAutoNumberInsert(true);
            database.setLinkResolver((ignored, target) -> {
                throw new IOException("Controlled catalog edit may not resolve targets");
            });
            Table catalog = database.getSystemTable("MSysObjects");
            Row selected = null;
            for (Row row : catalog) {
                if (arguments[3].equals(row.get("Name"))) {
                    if (selected != null) {
                        throw new IllegalStateException("Ambiguous controlled alias");
                    }
                    selected = row;
                }
            }
            if (selected == null) {
                throw new IllegalStateException("Unknown controlled alias");
            }
            switch (arguments[4]) {
                case "provider" -> selected.put("Connect", "UNKNOWN;token=inert");
                case "odbc" -> {
                    selected.put("Type", (short) 4);
                    selected.put("Connect", "ODBC;DSN=inert");
                }
                case "database" -> selected.put("Database", "unexpected.accdb");
                case "chain" -> selected.put("ForeignName", "Remote chain");
                case "identity" -> selected.put("Id", Integer.MIN_VALUE + 12345);
                default -> throw new IllegalArgumentException("Unknown controlled metadata edit");
            }
            catalog.updateRow(selected);
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 5 && arguments[0].equals("--metadata-copy")) {
            metadataCopy(arguments);
            return;
        }
        if (arguments.length != 2 && arguments.length != 3) {
            throw new IllegalArgumentException("Expected fresh directory, frontend format and optional backend format");
        }
        Path directory = Path.of(arguments[0]);
        Files.createDirectory(directory);
        Database.FileFormat frontendFormat = Database.FileFormat.valueOf(arguments[1]);
        Database.FileFormat backendFormat = arguments.length == 3
                ? Database.FileFormat.valueOf(arguments[2]) : frontendFormat;
        Path backend = directory.resolve("backend" + suffix(backendFormat));
        Path frontend = directory.resolve("frontend" + suffix(frontendFormat));
        try (Database database = new DatabaseBuilder(backend).setFileFormat(backendFormat).create()) {
            database.setEvaluateExpressions(false);
            Table people = new TableBuilder("Remote People")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(new ColumnBuilder("Team", DataType.LONG))
                    .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(100))
                    .toTable(database);
            people.addRow(1, 10, "alpha");
            people.addRow(2, 20, "beta");
            people.addRow(3, null, null);
            people.addRow(4, 10, "é中");
            people.addRow(5, 20, "ALPHA");
            people.addRow(6, 30, "");
            Table orders = new TableBuilder("Remote Orders")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(new ColumnBuilder("Person", DataType.LONG))
                    .addColumn(new ColumnBuilder("Units", DataType.LONG))
                    .toTable(database);
            orders.addRow(101, 1, 2);
            orders.addRow(102, 1, 3);
            orders.addRow(103, 2, null);
            orders.addRow(104, 4, 4);
            orders.addRow(105, 99, 5);
            orders.addRow(106, null, 6);
            new TableBuilder("Remote Empty")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(database);
            database.createLinkedTable("Remote chain", "never-open.accdb", "Never");
        }
        try (Database database = new DatabaseBuilder(frontend).setFileFormat(frontendFormat).create()) {
            database.setEvaluateExpressions(false);
            Table rates = new TableBuilder("Rates")
                    .addColumn(new ColumnBuilder("Team", DataType.LONG))
                    .addColumn(new ColumnBuilder("Rate", DataType.LONG))
                    .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(100))
                    .toTable(database);
            rates.addRow(10, 2, "alpha");
            rates.addRow(20, 3, "BETA");
            rates.addRow(40, 4, "orphan");
            for (String[] mapping : new String[][] {
                    {"People link", "Remote People"}, {"People alias", "Remote People"},
                    {"Orders link", "Remote Orders"}, {"Empty link", "Remote Empty"}}) {
                database.createLinkedTable(mapping[0], backend.getFileName().toString(), mapping[1]);
            }
            database.createLinkedTable("Unmapped link", "never-open.accdb", "Never");
            // Saved SELECT fragments are inert syntax fixtures, not evidence that
            // the reference writer or Access executed a linked query.
            database.setAllowAutoNumberInsert(true);
            savedRoot(database);
            database.flush();
            if (database.getQueries().size() != 1) {
                throw new IllegalStateException("Unexpected inert saved query count");
            }
        }
    }
}
