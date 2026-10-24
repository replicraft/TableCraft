import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert metadata fixtures and independent Jackcess getter observations. */
public class IndexColumnsFixture {
    private static void describe(Database database) throws Exception {
        for (String name : database.getTableNames()) {
            Table table = database.getTable(name);
            for (Index index : table.getIndexes()) {
                StringBuilder result = new StringBuilder(name).append('\t').append(index.getName());
                for (Index.Column column : index.getColumns()) {
                    result.append('\t').append(column.getName()).append(':').append(column.isAscending());
                }
                System.out.println(result);
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2000");
        }
        Path path = Path.of(arguments[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output already exists");
        }
        Database.FileFormat format = switch (arguments[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2000" -> Database.FileFormat.V2000;
            default -> throw new IllegalArgumentException("Unsupported format");
        };

        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            for (int width : new int[] {1, 2, 10}) {
                for (int count : new int[] {1, 24}) {
                    TableBuilder table = new TableBuilder("Columns" + width + "Indexes" + count);
                    for (int column = 0; column < 10; column++) {
                        table.addColumn(new ColumnBuilder("K" + column, DataType.LONG));
                    }
                    for (int index = 0; index < count; index++) {
                        IndexBuilder definition = new IndexBuilder("Index" + index);
                        for (int column = 0; column < width; column++) {
                            int number = (column + index) % 10;
                            definition.addColumns((column + index) % 2 == 0, "K" + number);
                        }
                        table.addIndex(definition);
                    }
                    table.toTable(database).addRow(0, 1, 2, 3, 4, 5, 6, 7, 8, 9);
                }
            }
            describe(database);
        }
    }
}
