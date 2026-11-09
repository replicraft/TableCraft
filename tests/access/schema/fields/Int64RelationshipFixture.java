import com.healthmarketscience.jackcess.*;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert exact-integer relationship fixture; fresh outputs only. */
public class Int64RelationshipFixture {
  interface Attempt { void run() throws Exception; }
  static void rejects(Attempt action) throws Exception {
    try { action.run(); } catch (ConstraintViolationException expected) { return; }
    throw new AssertionError("Expected constraint rejection");
  }
  static Table pair(Database db, String name, String first, String second, boolean unique) throws Exception {
    TableBuilder builder = new TableBuilder(name)
      .addColumn(new ColumnBuilder(first, DataType.LONG))
      .addColumn(new ColumnBuilder(second, DataType.BIG_INT));
    if (unique) builder.addIndex(new IndexBuilder("Pair").addColumns(first, second).setUnique());
    return builder.toTable(db);
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path = Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format = switch (args[1]) {
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table parent = new TableBuilder("BigParents")
        .addColumn(new ColumnBuilder("Key", DataType.BIG_INT))
        .addIndex(new IndexBuilder("BigUnique").addColumns("Key").setUnique()).toTable(db);
      Table child = new TableBuilder("BigChildren")
        .addColumn(new ColumnBuilder("ForeignKey", DataType.BIG_INT)).toTable(db);
      new RelationshipBuilder(parent, child).addColumns("Key", "ForeignKey")
        .setReferentialIntegrity().setName("BigKeys").toRelationship(db);
      Long[] values = {Long.MIN_VALUE, Long.MAX_VALUE, 9007199254740992L, 9007199254740993L, 0L, null, null};
      for (Long value : values) parent.addRow(value);
      for (Long value : values) child.addRow(value);
      rejects(() -> child.addRow(9007199254740994L));
      Table unique = new TableBuilder("BigUniqueChildren")
        .addColumn(new ColumnBuilder("ForeignKey", DataType.BIG_INT))
        .addIndex(new IndexBuilder("BigUnique").addColumns("ForeignKey").setUnique()).toTable(db);
      new RelationshipBuilder(parent, unique).addColumns("Key", "ForeignKey")
        .setReferentialIntegrity().setName("UniqueBigKeys").toRelationship(db);
      for (Long value : new Long[] {Long.MIN_VALUE, Long.MAX_VALUE, null, null}) unique.addRow(value);
      rejects(() -> unique.addRow(Long.MIN_VALUE));
      Table mixedParent = pair(db, "MixedParents", "A", "B", true);
      Table mixedChild = pair(db, "MixedChildren", "X", "Y", false);
      new RelationshipBuilder(mixedParent, mixedChild).addColumns("A", "X").addColumns("B", "Y")
        .setReferentialIntegrity().setName("MixedKeys").toRelationship(db);
      Object[][] pairs = {{1, Long.MIN_VALUE}, {1, Long.MAX_VALUE}, {2, 9007199254740993L},
        {null, 9007199254740993L}, {4, null}, {null, null}, {null, null}};
      for (Object[] valuesPair : pairs) mixedParent.addRow(valuesPair);
      for (Object[] valuesPair : pairs) mixedChild.addRow(valuesPair);
      rejects(() -> mixedChild.addRow(2, 9007199254740992L));
      rejects(() -> mixedChild.addRow(null, Long.MAX_VALUE));
      rejects(() -> mixedChild.addRow(2, null));
      Table mixedUnique = pair(db, "MixedUniqueChildren", "X", "Y", true);
      new RelationshipBuilder(mixedParent, mixedUnique).addColumns("A", "X").addColumns("B", "Y")
        .setReferentialIntegrity().setName("UniqueMixedKeys").toRelationship(db);
      mixedUnique.addRow(null, 9007199254740993L);
      mixedUnique.addRow(null, null);
      mixedUnique.addRow(null, null);
      rejects(() -> mixedUnique.addRow(null, 9007199254740993L));
    }
    System.out.println("All Int64/mixed tuple acceptance and rejection assertions passed.");
  }
}
