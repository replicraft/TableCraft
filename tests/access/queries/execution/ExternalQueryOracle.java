import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;
import java.util.concurrent.atomic.AtomicInteger;

/** Original local-table getters only; never resolves links or executes SQL. */
public class ExternalQueryOracle {
    private static String text(Object value) {
        return value == null ? "N" : "T" + Base64.getEncoder().encodeToString(
                value.toString().getBytes(StandardCharsets.UTF_8));
    }

    private static void records(Database database, String scope, String name) throws Exception {
        Table table = database.getTable(name);
        for (var index : table.getIndexes()) {
            for (var column : index.getColumns()) {
                System.out.printf("X\t%s\t%s\t%s\t%s\t%b%n", scope, text(name), text(index.getName()),
                        text(column.getName()), column.isAscending());
            }
        }
        for (var column : table.getColumns()) {
            if (column.getType() == DataType.TEXT) {
                var order = ((ColumnImpl) column).getTextSortOrder();
                System.out.printf("P\t%s\t%s\t%s\t%d\t%d%n", scope, text(name), text(column.getName()),
                        order.getValue(), order.getVersion());
            }
        }
        for (Row row : table) {
            StringBuilder result = new StringBuilder("R\t" + scope + "\t" + text(name));
            for (var column : table.getColumns()) {
                Object value = row.get(column.getName());
                result.append('\t').append(value instanceof Number ? "I" + value : text(value));
            }
            System.out.println(result);
        }
    }

    public static void main(String[] arguments) throws Exception {
        AtomicInteger opens = new AtomicInteger();
        try (Database root = new DatabaseBuilder(Path.of(arguments[0])).setReadOnly(true).open();
                Database target = new DatabaseBuilder(Path.of(arguments[1])).setReadOnly(true).open()) {
            for (Database database : new Database[] {root, target}) {
                database.setEvaluateExpressions(false);
                database.setLinkResolver((ignored, token) -> {
                    opens.incrementAndGet();
                    throw new IOException("Oracle target traversal is forbidden");
                });
            }
            for (String name : new String[] {"Rates", "Remote People", "Remote Orders", "Root Only"}) {
                records(root, "root", name);
            }
            for (String name : new String[] {"Rates", "Remote People", "Remote Orders", "Remote Empty"}) {
                records(target, "target", name);
            }
        }
        if (opens.get() != 0) {
            throw new IllegalStateException("Getter oracle followed a database link");
        }
        System.out.println("O\t0");
    }
}
