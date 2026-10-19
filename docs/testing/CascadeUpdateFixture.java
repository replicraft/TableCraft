import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

/** Original inert recipe for bounded update-action graphs; fresh paths only. */
public class CascadeUpdateFixture {
  static Table key(Database db, String name) throws Exception {
    Table t = new TableBuilder(name).addColumn(new ColumnBuilder("Key", DataType.LONG))
      .addIndex(new IndexBuilder("Primary").addColumns("Key").setPrimaryKey()).toTable(db);
    for (int key : new int[]{1,2,3}) t.addRow(key);
    return t;
  }
  static Table tuple(Database db, String name, boolean unique) throws Exception {
    var builder = new TableBuilder(name).addColumn(new ColumnBuilder("Id", DataType.LONG))
      .addColumn(new ColumnBuilder("A", DataType.LONG)).addColumn(new ColumnBuilder("B", DataType.BIG_INT));
    var index = new IndexBuilder("Tuple").addColumns("A").addColumns(false,"B");
    if (unique) index.setUnique();
    return builder.addIndex(index).toTable(db);
  }
  static void relation(Database db, Table parent, Table child, String name,
      String parentColumn, String childColumn, boolean cascade) throws Exception {
    var relation = new RelationshipBuilder(parent,child).addColumns(parentColumn,childColumn)
      .setReferentialIntegrity().setName(name);
    if (cascade) relation.setCascadeUpdates();
    relation.toRelationship(db);
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Fresh path and format required");
    var format = switch(args[1]) { case "V2016" -> Database.FileFormat.V2016; case "V2019" -> Database.FileFormat.V2019; default -> throw new IllegalArgumentException("Format"); };
    try (Database db = new DatabaseBuilder(Path.of(args[0])).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      Table roots=key(db,"Roots"), middle=key(db,"Middle");
      Table leaves=new TableBuilder("Leaves").addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("Parent",DataType.LONG)).addColumn(new ColumnBuilder("Notes",DataType.MEMO))
        .addColumn(new ColumnBuilder("Payload",DataType.OLE)).toTable(db);
      for (int i=0;i<8;i++) { byte[] payload=new byte[i==0?12288:16]; Arrays.fill(payload,(byte)i);
        leaves.addRow(i+1,i==7?null:(i%3)+1,i==0?"Unicode é🦀 ".repeat(700):"row "+i,payload); }
      relation(db,roots,middle,"RootMiddle","Key","Key",true);
      relation(db,middle,leaves,"MiddleLeaf","Key","Parent",true);
      relation(db,roots,leaves,"RootLeaf","Key","Parent",true);
      Table ca=key(db,"CycleA"), cb=key(db,"CycleB");
      relation(db,ca,cb,"CycleAB","Key","Key",true);
      relation(db,cb,ca,"CycleBA","Key","Key",true);
      Table restricted=key(db,"Restricted"), references=key(db,"References");
      relation(db,restricted,references,"RestrictLink","Key","Key",false);
      Table tr=tuple(db,"TupleRoots",true), tm=tuple(db,"TupleMiddle",true), tl=tuple(db,"TupleLeaves",false);
      for (int i=1;i<=3;i++) { tr.addRow(i,i,(long)i*10); tm.addRow(i,i,(long)i*10); tl.addRow(i,i,(long)i*10); }
      tl.addRow(4,1,10L); tl.addRow(5,null,null);
      new RelationshipBuilder(tr,tm).addColumns("A","A").addColumns("B","B").setReferentialIntegrity().setCascadeUpdates().setName("TupleRM").toRelationship(db);
      new RelationshipBuilder(tm,tl).addColumns("A","A").addColumns("B","B").setReferentialIntegrity().setCascadeUpdates().setName("TupleML").toRelationship(db);
    }
  }
}
