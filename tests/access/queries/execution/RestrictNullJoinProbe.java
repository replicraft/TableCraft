import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.util.Joiner;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.TimeZone;

/** Original read-only public Joiner diagnostic; no deletion or database generation. */
public final class RestrictNullJoinProbe {
    public static void main(String[] arguments) throws Exception {
        Path path = Path.of(arguments[0]);
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setEnforceForeignKeys(true);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("No linked targets authorized");
            });
            for (String prefix : List.of("I32", "IntegerTuple", "DecimalTuple")) {
                Table parent = database.getTable(prefix + "Parents");
                for (String childName : prefix.equals("I32")
                        ? List.of("I32Children", "I32SecondChildren")
                        : List.of(prefix + "Children")) {
                    Table child = database.getTable(childName);
                    Joiner joiner = Joiner.create(parent, child);
                    for (Row row : parent) {
                        if (row.getInt("ID") == 6) {
                            System.out.printf("JOIN\t%s\t%s\tparentID6\t%s%n",
                                parent.getName(), child.getName(), joiner.hasRows(row));
                            for (Row found : joiner.findRows(row)) {
                                System.out.printf("MATCH\t%s\t%s\t%d%n", parent.getName(),
                                    child.getName(), found.getInt("ID"));
                            }
                        }
                    }
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
        if (!java.util.Arrays.equals(before, Files.readAllBytes(path))) {
            throw new AssertionError("Read-only diagnostic changed input bytes");
        }
        System.out.println("UNCHANGED\t" + HexFormat.of().formatHex(
            MessageDigest.getInstance("SHA-256").digest(before)));
    }
}
