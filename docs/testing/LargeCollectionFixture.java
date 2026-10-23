import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import java.nio.file.Files;
import java.nio.file.Path;

// Original inert whole-table collection scale recipe, including bounded refusals.
public class LargeCollectionFixture {
    private static Table table(Database database, String name) throws Exception {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
            .addColumn(new ColumnBuilder("Numbers").setMultiValue(DataType.LONG).setValueList("0", "1"))
            .toTable(database);
    }

    private static ComplexValueForeignKey parent(Table table) throws Exception {
        Object[] row = table.addRow(Column.AUTO_NUMBER, Column.AUTO_NUMBER);
        return (ComplexValueForeignKey) table.getColumn("Numbers").getRowValue(row);
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh output/format or --inspect path");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
        }
        if (Files.exists(Path.of(args[0]))) {
            throw new IllegalArgumentException("Output must not exist");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2007" -> Database.FileFormat.V2007;
            default -> throw new IllegalArgumentException("Unsupported engineering format");
        };
        try (Database database = new DatabaseBuilder(Path.of(args[0]))
                .setFileFormat(format).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            Table parents = table(database, "ManyParents");
            for (int id = 1; id <= 20_000; id++) {
                ComplexValueForeignKey key = parent(parents);
                if (id % 5 != 0) {
                    key.addMultiValue(id);
                    key.addMultiValue(null);
                    key.addMultiValue(id);
                }
            }

            Table children = table(database, "ManyChildren");
            for (int id = 1; id <= 400; id++) {
                ComplexValueForeignKey key = parent(children);
                for (int child = 0; child < 100; child++) {
                    key.addMultiValue(child % 17 == 0 ? null : child % 7);
                }
            }

            ComplexValueForeignKey overflow = parent(table(database, "Overflow"));
            for (int value = 0; value < 101; value++) {
                overflow.addMultiValue(value);
            }
        }
        System.out.println("Created 20,000 parents/48,000 children and 400 parents/40,000 children.");
    }

    private static void inspect(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            for (String name : new String[] {"ManyParents", "ManyChildren", "Overflow"}) {
                for (Row row : database.getTable(name)) {
                    var key = (ComplexValueForeignKey) row.get("Numbers");
                    StringBuilder line = new StringBuilder(name).append('\t').append(row.getInt("ID"));
                    for (var value : key.getMultiValues()) {
                        line.append('\t').append(value.get() == null ? "N" : value.get());
                    }
                    System.out.println(line);
                }
            }
        }
    }

}
