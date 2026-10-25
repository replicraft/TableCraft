import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.TableMetaData;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicInteger;

/** Reopened public metadata getters; target resolution is an explicit failing trap. */
public class LinkedTableOracle {
    static String text(Object value) {
        if (value == null) {
            return "N";
        }
        byte[] bytes = value.toString().getBytes(StandardCharsets.UTF_8);
        return "T" + Base64.getEncoder().encodeToString(bytes);
    }

    public static void main(String[] args) throws Exception {
        AtomicInteger targetOpens = new AtomicInteger();
        try (Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            db.setEvaluateExpressions(false);
            db.setLinkResolver((ignored, target) -> {
                targetOpens.incrementAndGet();
                throw new IOException("Target resolution is forbidden");
            });
            for (TableMetaData metadata : db.newTableMetaDataIterable()) {
                if (!metadata.isLinked()) {
                    continue;
                }
                Row selected = null;
                for (Row row : db.getSystemTable("MSysObjects")) {
                    if (metadata.getName().equals(row.get("Name"))) {
                        if (selected != null) {
                            throw new IllegalStateException("Ambiguous linked identity");
                        }
                        selected = row;
                    }
                }
                if (selected == null) {
                    throw new IllegalStateException("Missing linked identity");
                }
                if (!Objects.equals(metadata.getLinkedDbName(), selected.get("Database"))
                        || !Objects.equals(metadata.getLinkedTableName(), selected.get("ForeignName"))
                        || !Objects.equals(metadata.getConnectionName(), selected.get("Connect"))) {
                    throw new IllegalStateException("Getter/raw metadata mismatch");
                }
                System.out.println(text(metadata.getName()) + "\t" + selected.get("Id")
                    + "\t" + metadata.getType() + "\t" + text(metadata.getLinkedDbName())
                    + "\t" + text(metadata.getLinkedTableName()) + "\t" + text(metadata.getConnectionName()));
            }
        }
        if (targetOpens.get() != 0) {
            throw new IllegalStateException("Metadata opened a target");
        }
    }
}
