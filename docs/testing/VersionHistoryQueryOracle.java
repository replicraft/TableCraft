import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.complex.VersionHistoryColumnInfoImpl;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.HexFormat;

/** Read-only declarations/current values; history records use VersionHistoryOracle. */
public class VersionHistoryQueryOracle {
    static String text(String value) {
        return value == null ? "N" : "T" + HexFormat.of().formatHex(
            value.getBytes(StandardCharsets.UTF_8));
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            throw new IllegalArgumentException("Expected path, table, visible Memo");
        }

        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable(args[1]);
            Column memo = table.getColumn(args[2]);
            Column hidden = memo.getVersionHistoryColumn();
            if (hidden == null) {
                throw new IllegalArgumentException("Memo has no stored history");
            }
            var info = (VersionHistoryColumnInfoImpl) hidden.getComplexInfo();
            System.out.println("D\t" + text(hidden.getName()) + "\t"
                + text(info.getValueColumn().getName()) + "\t"
                + text(info.getModifiedDateColumn().getName()));

            int ordinal = 0;
            for (Row row : table) {
                System.out.println("P\t" + ordinal++ + "\tI32:" + row.get("ID")
                    + "\t" + text((String) row.get(memo.getName())));
            }
        }
    }
}
