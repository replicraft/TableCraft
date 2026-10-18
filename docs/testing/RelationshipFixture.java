import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.RelationshipImpl;
import java.nio.file.*;

// Original repository-licensed inert relationship recipe; fresh outputs only.
public class RelationshipFixture {
  interface Attempt { void run() throws Exception; }
  static void rejects(Attempt attempt) throws Exception {
    try { attempt.run(); } catch (ConstraintViolationException expected) { return; }
    throw new IllegalStateException("Expected foreign-key rejection");
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019, or --inspect path");
    if (args[0].equals("--inspect")) { inspect(Path.of(args[1])); return; }
    Path path=Path.of(args[0]);
    if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]) {
      case "V2000" -> Database.FileFormat.V2000;
      case "V2007" -> Database.FileFormat.V2007;
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;
      default -> throw new IllegalArgumentException("Unsupported format");
    };
    try (Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table parent=new TableBuilder("Parents").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addIndex(new IndexBuilder("Key").addColumns("ID").setPrimaryKey()).toTable(db);
      Table child=new TableBuilder("Children").addColumn(new ColumnBuilder("ParentID",DataType.LONG)).toTable(db);
      new RelationshipBuilder(parent,child).addColumns("ID","ParentID").setReferentialIntegrity()
        .setCascadeUpdates().setCascadeDeletes().setName("ParentChildren").toRelationship(db);
      parent.addRow(1);parent.addRow(2);child.addRow(1);child.addRow(1);child.addRow((Object)null);

      Table unique=new TableBuilder("UniqueChildren").addColumn(new ColumnBuilder("ParentID",DataType.LONG))
        .addIndex(new IndexBuilder("UniqueKey").addColumns("ParentID").setUnique()).toTable(db);
      new RelationshipBuilder(parent,unique).addColumns("ID","ParentID").setReferentialIntegrity().setName("OneToOne").toRelationship(db);
      unique.addRow(2);

      Table nullable=new TableBuilder("NullableChildren").addColumn(new ColumnBuilder("ParentID",DataType.LONG)).toTable(db);
      new RelationshipBuilder(parent,nullable).addColumns("ID","ParentID").setReferentialIntegrity()
        .setCascadeNullOnDelete().setName("SetNull").toRelationship(db);
      nullable.addRow(1);

      Table composite=new TableBuilder("CompositeParents").addColumn(new ColumnBuilder("A",DataType.LONG))
        .addColumn(new ColumnBuilder("B",DataType.LONG)).addIndex(new IndexBuilder("Pair").addColumns("A","B").setUnique()).toTable(db);
      Table dependent=new TableBuilder("CompositeChildren").addColumn(new ColumnBuilder("X",DataType.LONG))
        .addColumn(new ColumnBuilder("Y",DataType.LONG)).toTable(db);
      new RelationshipBuilder(composite,dependent).addColumns("A","X").addColumns("B","Y")
        .setReferentialIntegrity().setName("Composite").toRelationship(db);
      composite.addRow(1,10);composite.addRow(2,20);dependent.addRow(1,10);
      rejects(() -> dependent.addRow(null,10));
      rejects(() -> dependent.addRow(1,null));
      dependent.addRow(null,null);

      Table loose=new TableBuilder("LooseChildren").addColumn(new ColumnBuilder("ParentID",DataType.LONG)).toTable(db);
      new RelationshipBuilder(parent,loose).addColumns("ID","ParentID").setJoinType(Relationship.JoinType.LEFT_OUTER)
        .setName("Unenforced").toRelationship(db);
      loose.addRow(999);
    }
    inspect(path);
  }
  static String action(boolean cascade,boolean setNull) { return cascade?"Cascade":setNull?"SetNull":"Restrict"; }
  static void inspect(Path path) throws Exception {
    try (Database db=new DatabaseBuilder(path).setReadOnly(true).open()) {
      // This API includes user and system declarations. getRelationships()
      // excludes the built-in navigation relationships in newer templates.
      for (Relationship relationship:db.getSystemRelationships()) {
        int flags=((RelationshipImpl)relationship).getFlags();
        for (int ordinal=0;ordinal<relationship.getFromColumns().size();ordinal++) {
          System.out.printf("%s\t%s\t%s\t0x%08x\t%s\t%s\t%s\t%s\t%d\t%s\t%s%n",
            relationship.getName(),relationship.getFromTable().getName(),relationship.getToTable().getName(),flags,
            relationship.hasReferentialIntegrity()?"TRUE":"FALSE",relationship.isOneToOne()?"TRUE":"FALSE",
            action(relationship.cascadeUpdates(),false),action(relationship.cascadeDeletes(),relationship.cascadeNullOnDelete()),ordinal,
            relationship.getFromColumns().get(ordinal).getName(),relationship.getToColumns().get(ordinal).getName());
        }
      }
    }
  }
}
