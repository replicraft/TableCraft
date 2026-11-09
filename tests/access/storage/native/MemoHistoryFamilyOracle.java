import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.impl.complex.VersionHistoryColumnInfoImpl;
import java.nio.file.Path;

/** Original read-only flag/property oracle; text/history values use existing original getters. */
public class MemoHistoryFamilyOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected original path");
        }
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            for (String[] selected : new String[][] {
                {"Histories", "Notes"}, {"OtherHistories", "Details"},
            }) {
                Column memo = database.getTable(selected[0]).getColumn(selected[1]);
                Column hidden = memo.getVersionHistoryColumn();
                VersionHistoryColumnInfoImpl history = (VersionHistoryColumnInfoImpl) hidden.getComplexInfo();
                Object format = memo.getProperties().getValue("TextFormat");
                if (format != null && !(format instanceof Byte)) {
                    throw new AssertionError("Expected original BYTE TextFormat declaration");
                }
                System.out.printf("D\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                    selected[0], memo.getName(), memo.isCompressedUnicode(), memo.isHyperlink(),
                    format == null ? "N" : Byte.toUnsignedInt((Byte) format), hidden.getName(),
                    history.getValueColumn().getName(), history.getValueColumn().isCompressedUnicode(),
                    history.getModifiedDateColumn().getName(), history.getValueColumn().getTable().getName());
            }
        }
    }
}
