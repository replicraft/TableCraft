import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;

/** Original inert Unicode source metadata; never executes SQL or active content. */
public class SavedUnicodeFixture extends SavedForestFixture {
  static int definition(String name) throws Exception {
    int id = nextId++;
    Map<String, Object> object = new HashMap<>();
    object.put("Id", id);
    object.put("ParentId", parent);
    object.put("Name", name);
    object.put("Type", (short) 5);
    object.put("Flags", 0);
    catalog.addRowFromMap(object);
    row(id, (byte) 1, null, (short) 1, null, null, 1);
    row(id, (byte) 3, null, (short) 0, null, null, 1);
    return id;
  }

  static String quoted(String name) {
    return name.startsWith("[") ? name : "[" + name + "]";
  }

  static void query(String name, String leftTable, String leftAlias,
                    String rightTable, String rightAlias, Short kind,
                    String derived) throws Exception {
    int id = definition(name);
    String left = leftAlias == null ? leftTable : leftAlias;
    String right = rightAlias == null ? rightTable : rightAlias;
    row(id, (byte) 5, derived, null, derived == null ? leftTable : null, leftAlias, 1);
    row(id, (byte) 5, null, null, rightTable, rightAlias, 2);
    row(id, (byte) 6, quoted(left) + ".[ID]", null, "左の値", null, 1);
    row(id, (byte) 6, quoted(right) + ".[ID]", null, "右の値", null, 2);
    if (kind != null) {
      row(id, (byte) 7, quoted(left) + ".[ID]=" + quoted(right) + ".[ID]", kind, left, right, 1);
    }
    row(id, (byte) 11, "[左の値]", null, "A", null, 1);
    row(id, (byte) 11, "[右の値]", null, "A", null, 2);
  }

  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try (Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);
      db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects");
      queries = db.getSystemTable("MSysQueries");
      for (String name : new String[] {"左側🦀", "右側"}) {
        Table table = new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(db);
        for (int value : name.equals("左側🦀") ? new int[] {1, 2, 4} : new int[] {2, 3, 4}) {
          table.addRow(value);
        }
      }
      query("UnicodeAliases", "AData", "左🦀", "BData", "右", (short) 2, null);
      query("UnicodeQuoted", "AData", "[左a]", "BData", "[右b]", (short) 1, null);
      query("UnicodeTables", "左側🦀", null, "右側", null, (short) 2, null);
      query("UnicodeComma", "AData", "左", "BData", "右", null, null);
      query("UnicodeDerived", "AData", "左", "BData", "右", (short) 1, "SELECT ID FROM AData");
      query("UnicodeScripts", "AData", "قائمة", "BData", "פריטים", (short) 3, null);
      query("UnicodeNested", "UnicodeAliases", "継承", "BData", "右", null, null);
      // The named dependency exposes its logical Unicode output name, not ID.
      int nested = nextId - 1;
      for (Row row : queries) {
        if (Integer.valueOf(nested).equals(row.get("ObjectId")) && Byte.valueOf((byte) 6).equals(row.get("Attribute"))) {
          if ("左の値".equals(row.get("Name1"))) {
            row.put("Expression", "[継承].[左の値]");
            queries.updateRow(row);
          }
        }
      }
      db.flush();
      if (db.getQueries().size() != 21) throw new AssertionError("Unexpected query inventory");
    }
    System.out.println("Unicode sources created; no SQL executed.");
  }
}
