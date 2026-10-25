import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableMetaData;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;
import java.util.concurrent.atomic.AtomicInteger;

/** Original raw getters only; a throwing resolver prevents all target traversal. */
public class LinkedQueryOracle {
    private static String text(Object value) {
        return value == null ? "N" : "T" + Base64.getEncoder().encodeToString(
                value.toString().getBytes(StandardCharsets.UTF_8));
    }

    private static void records(Database database, String name) throws Exception {
        Table table = database.getTable(name);
        for (var column : table.getColumns()) {
            if (column.getType() == DataType.TEXT) {
                var order = ((ColumnImpl) column).getTextSortOrder();
                System.out.printf("P\t%s\t%s\t%d\t%d%n", text(name), text(column.getName()),
                        order.getValue(), order.getVersion());
            }
        }
        for (Row row : table) {
            StringBuilder output = new StringBuilder("R\t" + text(name));
            for (var column : table.getColumns()) {
                Object value = row.get(column.getName());
                output.append('\t').append(value instanceof Number ? "I" + value : text(value));
            }
            System.out.println(output);
        }
    }

    public static void main(String[] arguments) throws Exception {
        AtomicInteger opens = new AtomicInteger();
        try (Database frontend = new DatabaseBuilder(Path.of(arguments[0])).setReadOnly(true).open();
                Database backend = new DatabaseBuilder(Path.of(arguments[1])).setReadOnly(true).open()) {
            for (Database database : new Database[] {frontend, backend}) {
                database.setEvaluateExpressions(false);
                database.setLinkResolver((ignored, target) -> {
                    opens.incrementAndGet();
                    throw new IOException("Oracle link traversal is forbidden");
                });
            }
            for (TableMetaData metadata : frontend.newTableMetaDataIterable()) {
                if (metadata.isLinked()) {
                    Row catalog = null;
                    for (Row row : frontend.getSystemTable("MSysObjects")) {
                        if (metadata.getName().equals(row.get("Name"))) {
                            if (catalog != null) {
                                throw new IllegalStateException("Ambiguous frontend link identity");
                            }
                            catalog = row;
                        }
                    }
                    if (catalog == null) {
                        throw new IllegalStateException("Missing frontend link identity");
                    }
                    System.out.printf("L\t%s\t%d\t%s\t%s\t%s%n", text(metadata.getName()),
                            catalog.getInt("Id"), text(metadata.getLinkedDbName()),
                            text(metadata.getLinkedTableName()), text(metadata.getConnectionName()));
                }
            }
            for (String name : new String[] {"Remote People", "Remote Orders", "Remote Empty", "Remote chain"}) {
                Row catalog = null;
                for (Row row : backend.getSystemTable("MSysObjects")) {
                    if (name.equals(row.get("Name"))) {
                        catalog = row;
                    }
                }
                if (catalog == null) {
                    throw new IllegalStateException("Missing target identity");
                }
                System.out.printf("D\t%s\t%d%n", text(name), catalog.getInt("Id"));
                if (!name.equals("Remote chain")) {
                    records(backend, name);
                }
            }
            records(frontend, "Rates");
            for (var query : frontend.getQueries()) {
                System.out.printf("Q\t%s\t%s%n", text(query.getName()), text(query.toSQLString()));
            }
        }
        if (opens.get() != 0) {
            throw new IllegalStateException("Oracle opened a linked target");
        }
        System.out.println("O\t0");
    }
}
