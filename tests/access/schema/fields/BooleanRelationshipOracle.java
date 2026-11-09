import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.Relationship;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.RelationshipImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Original read-only Boolean declaration/row oracle; no formulas or active content. */
public class BooleanRelationshipOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected original database path");
        }
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            List<String> names = new ArrayList<>(database.getTableNames());
            names.sort(String::compareTo);
            for (String name : names) {
                Table table = database.getTable(name);
                String columns = String.join(",", table.getColumns().stream().map(Column::getName).toList());
                System.out.printf("T\t%s\t%s%n", name, columns);
                for (Column column : table.getColumns()) {
                    System.out.printf("C\t%s\t%s\t%s\t%s%n", name, column.getName(),
                        column.getType(), scalar(column.getProperties().getValue("Required")));
                }
                for (Index index : table.getIndexes()) {
                    String keys = String.join(",", index.getColumns().stream()
                        .map(column -> column.getName() + (column.isAscending() ? ":A" : ":D")).toList());
                    System.out.printf("I\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                        name, index.getName(), index.isPrimaryKey(), index.isUnique(),
                        index.isForeignKey(), index.isRequired(), index.shouldIgnoreNulls(), keys);
                }
                int ordinal = 0;
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    StringBuilder output = new StringBuilder("R\t").append(name)
                        .append('\t').append(ordinal++)
                        .append('\t').append(location.getPageNumber())
                        .append('\t').append(location.getRowNumber());
                    for (Column column : table.getColumns()) {
                        output.append('\t').append(scalar(row.get(column.getName())));
                    }
                    System.out.println(output);
                }
            }
            for (Relationship relationship : database.getRelationships()) {
                for (int index = 0; index < relationship.getFromColumns().size(); index++) {
                    System.out.printf("L\t%s\t%s\t%s\t%d\t%s\t%s\t%s\t%s\t%d%n",
                        relationship.getName(), relationship.getFromTable().getName(),
                        relationship.getToTable().getName(), index,
                        relationship.getFromColumns().get(index).getName(),
                        relationship.getToColumns().get(index).getName(),
                        relationship.hasReferentialIntegrity(), relationship.isOneToOne(),
                        ((RelationshipImpl) relationship).getFlags());
                }
            }
        }
    }

    private static String scalar(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof Boolean flag) {
            return flag ? "B1" : "B0";
        }
        if (value instanceof Integer integer) {
            return "I32:" + integer;
        }
        throw new IllegalArgumentException("Unexpected original Boolean relationship carrier");
    }
}
