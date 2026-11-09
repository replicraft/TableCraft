import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;

/** Original same-table calculated dependency declarations and inert raw getters. */
public final class CalculatedDependencyFixture {
    private static final String[] TABLES = {"Chains", "EmptyChains"};

    private static Table table(Database database, String name) throws Exception {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Input", DataType.LONG))
            .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(32))
            .addColumn(new ColumnBuilder("Twice", DataType.LONG).setCalculatedInfo("[Input]*2"))
            .addColumn(new ColumnBuilder("Total", DataType.LONG).setCalculatedInfo("[twICE]+[Input]"))
            .addColumn(new ColumnBuilder("Diamond", DataType.LONG).setCalculatedInfo("[Twice]+[Total]"))
            .addColumn(new ColumnBuilder("Alias", DataType.LONG).setCalculatedInfo("[Total]"))
            .addColumn(new ColumnBuilder("Constant", DataType.LONG).setCalculatedInfo("7"))
            .addColumn(new ColumnBuilder("ConstantAlias", DataType.LONG).setCalculatedInfo("[Constant]+1"))
            .addColumn(new ColumnBuilder("Caption", DataType.TEXT).setLengthInUnits(128)
                .setCalculatedInfo("[Label]&':'&[Label]"))
            .addColumn(new ColumnBuilder("Title", DataType.TEXT).setLengthInUnits(128)
                .setCalculatedInfo("[Caption]&'!'"))
            .addColumn(new ColumnBuilder("Divide", DataType.LONG).setCalculatedInfo("[Input]\\([Input]-2)"))
            .addColumn(new ColumnBuilder("LateError", DataType.LONG).setCalculatedInfo("[Divide]+1"))
            .toTable(database);
    }

    private static void write(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            database.setEvaluateExpressions(false);
            Table table = table(database, TABLES[0]);
            Object[][] inputs = {{3, "é🦀"}, {-4, ""}, {null, null}, {0, "a\tb"}, {2, "quote\""}};
            for (int index = 0; index < inputs.length; index++) {
                table.addRow(index + 1, inputs[index][0], inputs[index][1],
                    999, 888, 777, 666, 555, 444, "stale caption", "stale title", 333, 222);
            }
            table(database, TABLES[1]);
        }
    }

    private static String value(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof String text) {
            return "T" + Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8));
        }
        if (value instanceof Integer integer) {
            return "I" + integer;
        }
        throw new IllegalArgumentException("Unexpected reference getter carrier");
    }

    private static void read(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked opens");
            });
            for (String name : TABLES) {
                Table table = database.getTable(name);
                System.out.println("TABLE\t" + name + "\t" + table.getRowCount());
                for (Column column : table.getColumns()) {
                    System.out.println("COLUMN\t" + name + "\t" + column.getName() + "\t" + column.getType()
                        + "\t" + column.isCalculated() + "\t"
                        + value(column.getProperties().getValue("Expression")) + "\t"
                        + String.valueOf(column.getProperties().getValue("ResultType")));
                }
                for (Row row : table) {
                    StringBuilder line = new StringBuilder("ROW\t").append(name);
                    for (Column column : table.getColumns()) {
                        line.append('\t').append(value(column.getRowValue(row)));
                    }
                    System.out.println(line);
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("write")) {
            write(Path.of(args[1]), args[2]);
        } else if (args.length == 2 && args[0].equals("read")) {
            read(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected write PATH FORMAT or read PATH");
        }
    }
}
