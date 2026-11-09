import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Path;

/** Original public-writer additions to freshly generated LinkedQueryFixture pairs. */
public class ExternalQueryFixture {
    private static Table people(Database database) throws Exception {
        return new TableBuilder("Remote People")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Team", DataType.LONG))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(100))
                .toTable(database);
    }

    public static void main(String[] arguments) throws Exception {
        try (Database root = DatabaseBuilder.open(Path.of(arguments[0]));
                Database target = DatabaseBuilder.open(Path.of(arguments[1]))) {
            root.setEvaluateExpressions(false);
            target.setEvaluateExpressions(false);
            if (root.getTableMetaData("Remote People") != null || target.getTableMetaData("Rates") != null) {
                throw new IllegalStateException("Expected an untouched fresh linked-query pair");
            }

            Table localPeople = people(root);
            localPeople.addRow(1, 40, "root-alpha");
            localPeople.addRow(7, 10, "root-seven");
            localPeople.addRow(8, 20, null);
            new IndexBuilder("RootOnlyIndex").addColumns("ID").addToTable(localPeople);
            new IndexBuilder("ExternalId").addColumns("ID").addToTable(target.getTable("Remote People"));

            Table localOrders = new TableBuilder("Remote Orders")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(new ColumnBuilder("Person", DataType.LONG))
                    .addColumn(new ColumnBuilder("Units", DataType.LONG))
                    .toTable(root);
            localOrders.addRow(901, 1, 70);
            localOrders.addRow(902, 7, 80);

            Table externalRates = new TableBuilder("Rates")
                    .addColumn(new ColumnBuilder("Team", DataType.LONG))
                    .addColumn(new ColumnBuilder("Rate", DataType.LONG))
                    .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(100))
                    .toTable(target);
            externalRates.addRow(10, 100, "alpha");
            externalRates.addRow(20, 200, "BETA");
            externalRates.addRow(30, 300, "remote");

            new TableBuilder("Root Only")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(root).addRow(77);
            root.flush();
            target.flush();
        }
    }
}
