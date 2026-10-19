import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

// Original inert Jet 4 multi-page index mutation recipe; no Access-generated content.
public class IndexUpdateFixture {
  public static void main(String[] args) throws Exception {
    if(args.length == 2 && args[0].equals("--inspect")) {
      try(Database db = new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
        for(Row row : db.getTable("Items")) {
          java.security.MessageDigest digest=java.security.MessageDigest.getInstance("SHA-256");
          digest.update(row.getString("Notes").getBytes(java.nio.charset.StandardCharsets.UTF_8));
          digest.update(row.getBytes("Payload"));
          System.out.printf("%d\t%d\t%s%n",row.getInt("Id"),row.getInt("Ordinal"),HexFormat.of().formatHex(digest.digest()));
        }
      }
      return;
    }
    if(args.length != 1) throw new IllegalArgumentException("Expected fresh output path");
    Path path = Path.of(args[0]);
    if(Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    try(Database db = new DatabaseBuilder(path).setFileFormat(Database.FileFormat.V2000).create()) {
      Table table = new TableBuilder("Items")
        .addColumn(new ColumnBuilder("Id", DataType.LONG))
        .addColumn(new ColumnBuilder("Ordinal", DataType.LONG))
        .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
        .addColumn(new ColumnBuilder("Payload", DataType.OLE))
        .addIndex(new IndexBuilder("PK_Items").addColumns("Id").setPrimaryKey())
        .addIndex(new IndexBuilder("IdDesc").addColumns(false, "Id").setUnique())
        .toTable(db);
      for(int i = 0; i < 1000; i++) {
        byte[] payload = new byte[i == 0 ? 12_288 : 16];
        Arrays.fill(payload, (byte)(i % 256));
        table.addRow(2 * (i + 1), i, i == 0 ? "Unicode é🦀 ".repeat(700) : "row " + i, payload);
      }
    }
  }
}
