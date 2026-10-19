import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;
import java.util.Arrays;

/** Original inert indexed integral/Currency fixture recipe; fresh outputs only. */
public class IntegralUpdateFixture {
  static Object value(DataType type, Long key) {
    if (key == null) return null;
    return switch (type) {
      case BYTE -> (byte) key.longValue();
      case INT -> (short) key.longValue();
      case BIG_INT -> key;
      case MONEY -> BigDecimal.valueOf(key, 4);
      default -> throw new IllegalArgumentException("Unsupported test type");
    };
  }
  static Table table(Database db, String name, DataType type, boolean unique, boolean required) throws Exception {
    var builder = new TableBuilder(name)
      .addColumn(new ColumnBuilder("Id", DataType.LONG))
      .addColumn(new ColumnBuilder("Score", type))
      .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
      .addColumn(new ColumnBuilder("Payload", DataType.OLE))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey());
    for (String label : new String[]{"Asc", "Desc", "Alias"}) {
      var index = new IndexBuilder(label).addColumns(!label.equals("Desc"), "Score");
      if (unique) index.setUnique();
      if (required) index.setRequired();
      builder.addIndex(index);
    }
    return builder.toTable(db);
  }
  static void row(Table table, DataType type, int ordinal, Long key) throws Exception {
    byte[] payload = new byte[ordinal == 0 ? 12288 : 16];
    Arrays.fill(payload, (byte)(ordinal % 256));
    table.addRow(ordinal + 1, value(type, key), ordinal == 0 ? "Unicode é🦀 ".repeat(700) : "row " + ordinal, payload);
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output exists");
    Database.FileFormat format = switch(args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      for (DataType type : new DataType[]{DataType.BYTE, DataType.INT, DataType.BIG_INT, DataType.MONEY}) {
        String name = type.name();
        long minimum = type == DataType.BYTE ? 0 : type == DataType.INT ? Short.MIN_VALUE : Long.MIN_VALUE;
        long maximum = type == DataType.BYTE ? 255 : type == DataType.INT ? Short.MAX_VALUE : Long.MAX_VALUE;
        Long[] keys = {null, minimum, maximum, 10L, 20L, 20L,
          type == DataType.BIG_INT || type == DataType.MONEY ? 9007199254740993L : 31L,
          type == DataType.BIG_INT || type == DataType.MONEY ? 9007199254740992L : 32L};
        Table leaves = table(db, name + "Leaves", type, false, false);
        for (int i = 0; i < keys.length; i++) row(leaves, type, i, keys[i]);
        Table tree = table(db, name + "Tree", type, false, false);
        for (int i = 0; i < 1000; i++) row(tree, type, i, i % 17 == 0 ? null : (long)(i % 31));
        Table unique = table(db, name + "Unique", type, true, false);
        for (int i = 0; i < 4; i++) row(unique, type, i, i == 0 ? null : i * 10L);
        Table required = table(db, name + "Required", type, false, true);
        for (int i = 0; i < 3; i++) row(required, type, i, (i+1)*10L);
      }
    }
    System.out.println("Created fresh exact integral/Currency index and long-value controls.");
  }
}
