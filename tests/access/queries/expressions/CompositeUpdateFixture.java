import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;
import java.util.*;

/** Original inert two-to-ten-column index mutation recipe; fresh outputs only. */
public class CompositeUpdateFixture {
  static final DataType[] TYPES = {DataType.BYTE, DataType.INT, DataType.LONG, DataType.BIG_INT, DataType.MONEY};
  static Object value(DataType type, Long key) {
    if (key == null) return null;
    return switch(type) {
      case BYTE -> (byte) key.longValue();
      case INT -> (short) key.longValue();
      case LONG -> (int) key.longValue();
      case BIG_INT -> key;
      case MONEY -> BigDecimal.valueOf(key, 4);
      default -> throw new IllegalArgumentException("Unexpected carrier");
    };
  }
  static byte[] payload(int row) {
    byte[] bytes = new byte[row == 0 ? 12288 : 16]; Arrays.fill(bytes, (byte)(row % 256)); return bytes;
  }
  static String notes(int row) { return row == 0 ? "Unicode é🦀 ".repeat(700) : "row " + row; }
  static Table tuples(Database db, String name, int columns, int count) throws Exception {
    var builder = new TableBuilder(name).addColumn(new ColumnBuilder("Id", DataType.LONG));
    for (int c = 0; c < columns; c++) builder.addColumn(new ColumnBuilder("C" + c, TYPES[c % TYPES.length]));
    builder.addColumn(new ColumnBuilder("Notes", DataType.MEMO)).addColumn(new ColumnBuilder("Payload", DataType.OLE))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey());
    for (String label : new String[]{"Mixed", "Reverse", "Alias"}) {
      var index = new IndexBuilder(label);
      for (int c = 0; c < columns; c++) index.addColumns((c % 2 == 0) != label.equals("Reverse"), "C" + c);
      builder.addIndex(index);
    }
    Table table = builder.toTable(db);
    Long[][] seeds = {
      {null, null, null, null, null},
      {0L, (long)Short.MIN_VALUE, (long)Integer.MIN_VALUE, Long.MIN_VALUE, Long.MIN_VALUE},
      {255L, (long)Short.MAX_VALUE, (long)Integer.MAX_VALUE, Long.MAX_VALUE, Long.MAX_VALUE},
      {20L, 10L, 20L, 9007199254740992L, 9007199254740993L},
      {20L, 10L, 20L, 9007199254740992L, 9007199254740993L},
      {null, 7L, 10L, 1L, -1L},
      {32L, null, 1L, null, 1L}
    };
    for (int row = 0; row < count; row++) {
      Object[] data = new Object[columns + 3]; data[0] = row + 1;
      for (int c = 0; c < columns; c++) {
        Long key;
        if (row < seeds.length) key = seeds[row][c % TYPES.length];
        else {
          key = row % 29 == c % 29 ? null : Long.valueOf(switch(c % TYPES.length) {
            case 0 -> (row * 37L) % 251;
            case 1 -> (row * 193L) % 60000 - 30000;
            case 2 -> row * 101L;
            case 3 -> 9007199254740992L + row * 19L;
            default -> -9007199254740993L + row * 23L;
          });
        }
        data[c + 1] = value(TYPES[c % TYPES.length], key);
      }
      data[columns + 1] = notes(row); data[columns + 2] = payload(row); table.addRow(data);
    }
    return table;
  }
  static Table pair(Database db, String name, boolean primary, boolean unique, boolean required) throws Exception {
    var builder = new TableBuilder(name).addColumn(new ColumnBuilder("Id", DataType.LONG))
      .addColumn(new ColumnBuilder("A", DataType.LONG)).addColumn(new ColumnBuilder("B", DataType.BIG_INT))
      .addColumn(new ColumnBuilder("Notes", DataType.MEMO)).addColumn(new ColumnBuilder("Payload", DataType.OLE));
    var index = new IndexBuilder("Pair").addColumns("A").addColumns(false, "B");
    if (primary) index.setPrimaryKey(); else builder.addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey());
    if (unique) index.setUnique(); if (required) index.setRequired();
    return builder.addIndex(index).toTable(db);
  }
  static void row(Table table, int ordinal, Integer a, Long b) throws Exception { table.addRow(ordinal + 1, a, b, notes(ordinal), payload(ordinal)); }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path = Path.of(args[0]); if (Files.exists(path)) throw new IllegalArgumentException("Output exists");
    Database.FileFormat format = switch(args[1]) {case "V2016" -> Database.FileFormat.V2016; case "V2019" -> Database.FileFormat.V2019; default -> throw new IllegalArgumentException("Unsupported format");};
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      tuples(db, "Ordinary", 5, 7); tuples(db, "Tree", 5, 1000); tuples(db, "Ten", 10, 7);
      Table unique = pair(db, "Unique", false, true, false);
      row(unique, 0, 1, 10L); row(unique, 1, 1, 20L); row(unique, 2, 2, null); row(unique, 3, null, null); row(unique, 4, null, null);
      Table required = pair(db, "Required", false, false, true); row(required, 0, 1, 10L); row(required, 1, 2, 20L);
      Table parents = pair(db, "Parents", true, false, false);
      row(parents, 0, 10, 100L); row(parents, 1, 20, 200L); row(parents, 2, 30, 300L); row(parents, 3, 20, 100L); row(parents, 4, 10, 200L);
      Table children = pair(db, "Children", false, false, false);
      row(children, 0, null, null); row(children, 1, 10, 100L); row(children, 2, 20, 200L);
      new RelationshipBuilder(parents, children).addColumns("A", "A").addColumns("B", "B")
        .setReferentialIntegrity().setName("PairLink").toRelationship(db);
    }
    System.out.println("Created inert mixed/ten-column, unique/required and composite foreign-index controls.");
  }
}
