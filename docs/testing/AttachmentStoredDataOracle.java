import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.complex.Attachment;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import com.healthmarketscience.jackcess.impl.complex.AttachmentColumnInfoImpl;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.Map;

/** Original read-only physical declaration and encoded getter cross-check. */
public class AttachmentStoredDataOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected original attachment fixture path");
        }

        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("Collections");
            AttachmentColumnInfoImpl info = (AttachmentColumnInfoImpl) table.getColumn("Files").getComplexInfo();
            Column data = info.getFileDataColumn();
            if (data.getType() != DataType.OLE || !data.getName().equals("FileData")) {
                throw new IllegalStateException("Expected actual native FileData OLE declaration");
            }
            System.out.println("D\tFileData\tOLE\tBinary");

            int parent = 0;
            for (Row row : table) {
                ComplexValueForeignKey key = (ComplexValueForeignKey) row.get("Files");
                Map<Integer, byte[]> raw = new HashMap<>();
                for (Row child : info.getRawValues(key.get())) {
                    Object value = child.get(data.getName());
                    if (value != null && !(value instanceof byte[])) {
                        throw new IllegalStateException("Expected raw binary carrier");
                    }
                    Integer identity = (Integer) child.get(info.getPrimaryKeyColumn().getName());
                    if (raw.containsKey(identity)) {
                        throw new IllegalStateException("Duplicate stored identity");
                    }
                    raw.put(identity, (byte[]) value);
                }

                for (Attachment attachment : key.getAttachments()) {
                    int identity = attachment.getId().get();
                    byte[] encoded = attachment.getEncodedFileData();
                    if (!raw.containsKey(identity) || !Arrays.equals(raw.remove(identity), encoded)) {
                        throw new IllegalStateException("Flat raw bytes and encoded getter disagree");
                    }
                    String value = encoded == null ? "N" : "X" + HexFormat.of().formatHex(encoded);
                    System.out.printf("C\t%d\t%d\t%s%n", parent, identity, value);
                }
                if (!raw.isEmpty()) {
                    throw new IllegalStateException("Unmatched flat records");
                }
                parent++;
            }
        }
    }
}
