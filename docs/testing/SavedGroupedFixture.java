import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;

/** Original inert grouped query metadata; no SQL or application content executes. */
public class SavedGroupedFixture extends SavedForestFixture {
  static final String KEY = "[O].[ID] Mod 2";
  static final String MATCH = "[I].[ID] Mod 2=[O].[ID] Mod 2";
  static final String COUNT = "(SELECT Count(*) FROM AData AS I WHERE " + MATCH + ")";
  static final String EXISTS = "EXISTS(SELECT I.ID FROM AData AS I WHERE " + MATCH + " AND I.ID>=2)";

  static int definition(String name, int family, int type) throws Exception {
    int id = nextId++;
    Map<String, Object> object = new HashMap<>();
    object.put("Id", id);
    object.put("ParentId", parent);
    object.put("Name", name);
    object.put("Type", (short) 5);
    object.put("Flags", family);
    catalog.addRowFromMap(object);
    row(id, (byte) 1, null, (short) type, null, null, 1);
    row(id, (byte) 3, null, (short) 0, null, null, 1);
    return id;
  }

  static int grouped(String name, String key, String measure, String having) throws Exception {
    int id = definition(name, 0, 1);
    row(id, (byte) 5, null, null, "AData", "O", 1);
    row(id, (byte) 6, key, null, "K", null, 1);
    row(id, (byte) 6, measure, null, "N", null, 2);
    row(id, (byte) 9, key, null, null, null, 1);
    if (having != null) {
      row(id, (byte) 10, having, null, null, null, 1);
    }
    row(id, (byte) 11, "K", null, "A", null, 1);
    return id;
  }

  static void projection(int id, String alias) throws Exception {
    row(id, (byte) 6, alias + ".K", null, "K", null, 1);
    row(id, (byte) 6, alias + ".N", null, "N", null, 2);
    row(id, (byte) 11, "K", null, "A", null, 1);
  }

  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try (Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);
      db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects");
      queries = db.getSystemTable("MSysQueries");
      grouped("GroupedScalar", KEY, COUNT, null);
      grouped("GroupedHaving", KEY, "Count(*)", EXISTS);
      grouped("GroupedIn", KEY, "Count(*)", KEY + " IN(SELECT I.ID Mod 2 FROM BData AS I)");
      grouped("GroupedAll", KEY, "Count(*)", KEY + " >= ALL(SELECT I.ID Mod 2 FROM BData AS I WHERE I.ID Mod 2=0)");
      grouped("GroupedNested", KEY, "(SELECT (SELECT " + KEY + "))", null);
      grouped("GroupedUnion", KEY, "Count(*)", KEY + " IN(SELECT " + KEY + " UNION ALL SELECT 99)");
      grouped("GroupedJoin", KEY,
          "(SELECT Count(*) FROM BData AS I INNER JOIN CData AS J ON I.ID=J.ID AND I.ID Mod 2=" + KEY + ")", null);
      int id = grouped("GroupedParams", "O.ID Mod [Divisor]",
          "(SELECT Count(*) FROM AData AS I WHERE I.ID Mod [Divisor]=O.ID Mod [Divisor])", null);
      row(id, (byte) 2, null, (short) 4, "Divisor", null, 1);

      id = definition("GroupedNamed", 0, 1);
      row(id, (byte) 5, null, null, "GroupedScalar", "G", 1);
      projection(id, "G");
      id = definition("GroupedDerived", 0, 1);
      row(id, (byte) 5, "SELECT " + KEY + " AS K," + COUNT + " AS N FROM AData AS O GROUP BY " + KEY,
          null, null, "D", 1);
      projection(id, "D");

      grouped("GroupedCardinality", KEY, "(SELECT I.ID FROM AData AS I)", null);
      grouped("GroupedBadCapture", KEY, "(SELECT O.ID)", null);
      grouped("GroupedBadJoin", KEY,
          "(SELECT Count(*) FROM BData AS I INNER JOIN CData AS J ON I.ID=J.ID AND J.ID=O.ID)", null);

      id = definition("GroupedCrosstab", 0x10, 6);
      row(id, (byte) 5, null, null, "AData", "O", 1);
      row(id, (byte) 6, KEY, (short) 2, "K", null, 1);
      row(id, (byte) 6, "Count(*)", (short) 2, "Total", null, 2);
      row(id, (byte) 6, "Count(*)", (short) 0, "Measure", null, 3);
      row(id, (byte) 6, "O.ID IN(1,2,4)", (short) 1, null, null, 4);
      row(id, (byte) 9, KEY, (short) 2, null, null, 1);
      row(id, (byte) 10, EXISTS, null, null, null, 1);
      db.flush();
      if (db.getQueries().size() != 28) {
        throw new AssertionError("Query count changed");
      }
    }
    System.out.println("Original grouped query definitions created without execution.");
  }
}
