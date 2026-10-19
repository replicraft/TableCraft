import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;
import java.util.*;

/** Original inert reciprocal integral-index update recipe; fresh outputs only. */
public class ForeignUpdateFixture {
  static Object value(DataType type, Long key) {
    if (key == null) return null;
    return switch (type) {
      case BYTE -> (byte) key.longValue();
      case INT -> (short) key.longValue();
      case LONG -> (int) key.longValue();
      case BIG_INT -> key;
      case MONEY -> BigDecimal.valueOf(key, 4);
      default -> throw new IllegalArgumentException("Unsupported test carrier");
    };
  }
  static void row(Table table, int ordinal, Object key) throws Exception {
    byte[] payload = new byte[ordinal == 0 ? 12288 : 16];
    Arrays.fill(payload, (byte)(ordinal % 256));
    table.addRow(ordinal + 1, key, ordinal == 0 ? "Unicode é🦀 ".repeat(700) : "row " + ordinal, payload);
  }
  static Table child(Database db, String name, DataType type, boolean unique, boolean required) throws Exception {
    var ascending = new IndexBuilder("ParentAlias").addColumns("Parent");
    var descending = new IndexBuilder("ParentDesc").addColumns(false, "Parent");
    if (unique) { ascending.setUnique(); descending.setUnique(); }
    return new TableBuilder(name).addColumn(new ColumnBuilder("Id", DataType.LONG))
      .addColumn(new ColumnBuilder("Parent", type).putProperty("Required", required))
      .addColumn(new ColumnBuilder("Notes", DataType.MEMO))
      .addColumn(new ColumnBuilder("Payload", DataType.OLE))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey())
      .addIndex(ascending).addIndex(descending).toTable(db);
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output exists");
    Database.FileFormat format = switch (args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      for (DataType type : new DataType[]{DataType.BYTE, DataType.INT, DataType.LONG, DataType.BIG_INT, DataType.MONEY}) {
        String name = type.name();
        long min = type == DataType.BYTE ? 0 : type == DataType.INT ? Short.MIN_VALUE : type == DataType.LONG ? Integer.MIN_VALUE : Long.MIN_VALUE;
        long max = type == DataType.BYTE ? 255 : type == DataType.INT ? Short.MAX_VALUE : type == DataType.LONG ? Integer.MAX_VALUE : Long.MAX_VALUE;
        long adjacent = type == DataType.BIG_INT || type == DataType.MONEY ? 9007199254740992L : 32;
        Table parents = new TableBuilder(name + "Parents").addColumn(new ColumnBuilder("Id", type))
          .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey()).toTable(db);
        for (long key : new long[]{min, max, 10, 20, 30, adjacent + (adjacent == 32 ? -1 : 1), adjacent}) parents.addRow(value(type, key));
        Long[] keys = {null, min, max, 10L, 20L, 20L, adjacent + (adjacent == 32 ? -1 : 1), adjacent};
        for (String suffix : new String[]{"Children", "One", "Required"}) {
          Table children = child(db, name + suffix, type, suffix.equals("One"), suffix.equals("Required"));
          int count = suffix.equals("Children") ? 1000 : suffix.equals("One") ? 3 : 1;
          for (int i = 0; i < count; i++) {
            Long key;
            if (suffix.equals("Children")) key = keys[i % keys.length];
            else if (suffix.equals("One")) key = i == 0 ? null : Long.valueOf(i * 10L);
            else key = 10L;
            row(children, i, value(type, key));
          }
          var relation = new RelationshipBuilder(parents, children).addColumns("Id", "Parent")
            .setReferentialIntegrity().setName(name + suffix + "Link");
          if (type == DataType.MONEY) relation.setCascadeUpdates().setCascadeDeletes();
          relation.toRelationship(db);
        }
      }
      Table self = new TableBuilder("SelfLinks").addColumn(new ColumnBuilder("Id", DataType.LONG))
        .addColumn(new ColumnBuilder("Parent", DataType.LONG))
        .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey()).toTable(db);
      self.addRow(1, null); self.addRow(2, 1); self.addRow(3, 1);
      new RelationshipBuilder(self, self).addColumns("Id", "Parent").setReferentialIntegrity().setName("SelfLink").toRelationship(db);
    }
    System.out.println("Created inert integral/Currency foreign-index, self, unique and cascade controls.");
  }
}
