import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.complex.ComplexValue;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import com.healthmarketscience.jackcess.impl.complex.ComplexColumnInfoImpl;
import com.healthmarketscience.jackcess.impl.complex.VersionHistoryColumnInfoImpl;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.HexFormat;

/** Original getter-only oracle; no imported expressions or actions execute. */
public class SignedComplexIdentityOracle {
    private static void identity(ComplexValue value, HashSet<Integer> seen) {
        if (value.getId().getRowId() == null || !seen.add(value.getId().get())) {
            throw new IllegalStateException("Persisted identity lost its physical row or uniqueness");
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (ComplexColumnInfoImpl.INVALID_ID.get() != -1
                || ComplexColumnInfoImpl.INVALID_ID.getRowId() != null) {
            throw new IllegalStateException("Pinned helper sentinel changed");
        }
        System.out.println("S\t-1\tnull-row-location");
        try (Database database = new DatabaseBuilder(Path.of(arguments[0]))
                .setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("Identities");
            String hidden = table.getColumn("Notes").getVersionHistoryColumn().getName();
            VersionHistoryColumnInfoImpl historyInfo =
                    (VersionHistoryColumnInfoImpl) table.getColumn(hidden).getComplexInfo();
            System.out.printf("D\t%s\t%s\t%s%n", hidden,
                    historyInfo.getValueColumn().getName(),
                    historyInfo.getModifiedDateColumn().getName());
            for (String name : new String[] {"Values", "Files", hidden}) {
                ComplexColumnInfoImpl<?> info =
                        (ComplexColumnInfoImpl<?>) table.getColumn(name).getComplexInfo();
                for (Row row : info.getPrimaryKeyColumn().getTable()) {
                    System.out.printf("R\t%s\t%d\t%d%n", name,
                            row.getInt(info.getPrimaryKeyColumn().getName()),
                            row.getInt(info.getComplexValueForeignKeyColumn().getName()));
                }
            }

            for (Row row : table) {
                int parent = row.getInt("ID");
                HashSet<Integer> seen = new HashSet<>();
                ComplexValueForeignKey values = (ComplexValueForeignKey) row.get("Values");
                for (var value : values.getMultiValues()) {
                    identity(value, seen);
                    System.out.printf("M\t%d\t%d\t%s%n", parent,
                            value.getId().get(), value.get() == null ? "NULL" : value.get());
                }
                seen.clear();
                ComplexValueForeignKey files = (ComplexValueForeignKey) row.get("Files");
                for (var value : files.getAttachments()) {
                    identity(value, seen);
                    System.out.printf("A\t%d\t%d\t%s\t%s\t%s%n", parent,
                            value.getId().get(), value.getFileName(),
                            HexFormat.of().formatHex(value.getEncodedFileData()),
                            HexFormat.of().formatHex(value.getFileData()));
                }
                seen.clear();
                ComplexValueForeignKey history = (ComplexValueForeignKey) row.get(hidden);
                for (var value : history.getVersions()) {
                    identity(value, seen);
                    System.out.printf("H\t%d\t%d\t%s%n", parent,
                            value.getId().get(), value.getValue());
                }
                System.out.printf("P\t%d\t%d\t%d\t%d%n", parent,
                        values.getMultiValues().size(), files.getAttachments().size(),
                        history.getVersions().size());
            }
        }
    }
}
