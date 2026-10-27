import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.TimeZone;

/** Own public-API action on a fresh copy; never regenerates registered inputs. */
public final class IndexSplitReference {
    private static final int EXPECTED_ID = 1;
    private static final int NEW_ID = Integer.MAX_VALUE;
    private static final int EXPECTED_ROWS = 500;
    private static final String TABLE = "Children";
    private static final String COLUMN = "ID";

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Expected SOURCE FRESH_OUTPUT");
        }
        Path source = Path.of(arguments[0]);
        Path output = Path.of(arguments[1]);
        if (Files.exists(output) || source.toRealPath().equals(output.toAbsolutePath().normalize())) {
            throw new IllegalArgumentException("Output must be fresh and distinct");
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            database.setLinkResolver((ignored, token) -> {
                throw new AssertionError("No linked target is authorized");
            });
            if (!database.isEnforceForeignKeys()) {
                throw new AssertionError("Foreign key enforcement must remain enabled");
            }
            for (String name : List.of("Parents", "Children", "Sentinel")) {
                Table table = database.getTable(name);
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                for (var column : table.getColumns()) {
                    if (column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected column properties");
                    }
                }
            }
            var relations = database.getRelationships();
            if (relations.size() != 1) {
                throw new AssertionError("Unexpected relationship population");
            }
            var relation = relations.get(0);
            if (!relation.hasReferentialIntegrity() || relation.cascadeUpdates()
                    || relation.cascadeDeletes() || relation.cascadeNullOnDelete()) {
                throw new AssertionError("Expected enforced RESTRICT relationship");
            }
            Table table = database.getTable(TABLE);
            var primary = table.getIndex("PrimaryID");
            if (table.getRowCount() != EXPECTED_ROWS || !primary.isPrimaryKey()
                    || !primary.isUnique() || !primary.isRequired()) {
                throw new AssertionError("Expected complete required-primary source");
            }
            Row selected = null;
            for (Row row : table) {
                if (row.getInt(COLUMN) == NEW_ID) {
                    throw new AssertionError("Replacement must be unused");
                }
                if (row.getInt(COLUMN) == EXPECTED_ID) {
                    if (selected != null) {
                        throw new AssertionError("Duplicate selected identity");
                    }
                    selected = row;
                }
            }
            if (selected == null || selected.getInt("ParentID") != EXPECTED_ID) {
                throw new AssertionError("Selected row/association mismatch");
            }
            selected.put(COLUMN, NEW_ID);
            table.updateRow(selected);
            System.out.println("updated=1; foreignKeys=true; expressions=false; timezone=UTC");
        }
    }
}
