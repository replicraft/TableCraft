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

/** Original inert IIf declarations, stale caches and ordinary reference getters. */
public final class CalculatedIIfFixture {
    private static final String[] TABLES = {"Choices", "EmptyChoices"};

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("This fixture must not open linked databases");
        });
    }

    private static ColumnBuilder number(String name, String expression) {
        return new ColumnBuilder(name, DataType.LONG).setCalculatedInfo(expression);
    }

    private static ColumnBuilder text(String name, String expression) {
        return new ColumnBuilder(name, DataType.TEXT)
            .setLengthInUnits(128).setCalculatedInfo(expression);
    }

    private static Table table(Database database, String name) throws Exception {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Input", DataType.LONG))
            .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(32))
            .addColumn(number("NumericIIf", "IIf([Input]>0,[Input]*2,0-[Input])"))
            .addColumn(number("NullableIIf", "IIf([Input] IS NULL,NULL,[Input]+1)"))
            .addColumn(new ColumnBuilder("BooleanIIf", DataType.BOOLEAN)
                .setCalculatedInfo("IIf([Input] IS NULL,True,[Input]>0)"))
            .addColumn(text("TextIIf", "IIf([Input] IS NULL,'missing',[Label])"))
            .addColumn(number("NullCondition", "IIf(NULL,[Input],7)"))
            .addColumn(number("Selected", "iIf([BooleanIIf],[NumericIIf],[NullableIIf])"))
            .addColumn(text("Title", "IIf([BooleanIIf],[TextIIf]&'!',[TextIIf]&'?')"))
            .addColumn(number("HiddenError", "IIf(True,[Input],1/0)"))
            // These are inert unsupported declarations, not qualified native formulas.
            .addColumn(number("HiddenDate", "IIf(True,[Input],Date())"))
            .addColumn(number("HiddenUnknown", "IIf(True,[Input],Mystery([ID]))"))
            .addColumn(number("HiddenIsNullCall", "IIf(True,[Input],IsNull([Input]))"))
            .addColumn(number("HiddenBadDependency", "IIf(True,[Input],[HiddenUnknown])"))
            .addColumn(number("TooFew", "IIf(True,[Input])"))
            .addColumn(number("TooMany", "IIf(True,[Input],0,1)"))
            .addColumn(number("Omitted", "IIf(True,,[Input])"))
            .addColumn(number("SelfCycle", "IIf(True,[Input],[SelfCycle])"))
            .addColumn(number("CycleA", "IIf(True,[Input],[CycleB])"))
            .addColumn(number("CycleB", "[CycleA]"))
            .toTable(database);
    }

    private static void write(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Refuse to overwrite an original database");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table table = table(database, TABLES[0]);
            Object[][] ordinary = {
                {3, "é🦀"},
                {-4, ""},
                {null, null},
                {0, null},
                {2, "tab\tquote\""},
            };
            for (int index = 0; index < ordinary.length; index++) {
                Object[] row = new Object[table.getColumnCount()];
                for (Column column : table.getColumns()) {
                    Object cached;
                    if (column.getType() == DataType.TEXT) {
                        cached = "stale cache";
                    } else if (column.getType() == DataType.BOOLEAN) {
                        cached = false;
                    } else {
                        cached = 999;
                    }
                    row[column.getColumnIndex()] = cached;
                }
                row[0] = index + 1;
                row[1] = ordinary[index][0];
                row[2] = ordinary[index][1];
                table.addRow(row);
            }
            table(database, TABLES[1]);
        }
    }

    private static String encode(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof Boolean flag) {
            return flag ? "B1" : "B0";
        }
        if (value instanceof Number) {
            return "I" + value;
        }
        if (value instanceof String text) {
            return "T" + Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8));
        }
        throw new IllegalArgumentException("Unexpected public getter carrier");
    }

    private static void read(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : TABLES) {
                Table table = database.getTable(name);
                System.out.println("TABLE\t" + name + "\t" + table.getRowCount());
                for (Column column : table.getColumns()) {
                    System.out.println("COLUMN\t" + name + "\t" + column.getName()
                        + "\t" + column.getType() + "\t" + column.isCalculated()
                        + "\t" + encode(column.getProperties().getValue("Expression"))
                        + "\t" + column.getProperties().getValue("ResultType"));
                }
                for (Row row : table) {
                    StringBuilder line = new StringBuilder("ROW\t").append(name);
                    for (Column column : table.getColumns()) {
                        line.append('\t').append(encode(column.getRowValue(row)));
                    }
                    System.out.println(line);
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handle activation");
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("write")) {
            write(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("read")) {
            read(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected write DATABASE PROFILE or read DATABASE");
        }
    }
}
