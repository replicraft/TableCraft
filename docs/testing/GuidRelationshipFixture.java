import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.Locale;

/** Original inert exact GUID/integer tuple recipe; no UUID generation or code runs. */
public class GuidRelationshipFixture {
  static final String ZERO="{00000000-0000-0000-0000-000000000000}";
  static final String A="{00112233-4455-6677-8899-AABBCCDDEEFF}";
  static final String B="{00112233-4455-6677-8899-AABBCCDDEE00}";
  static final String OTHER="{00112233-4455-6677-8899-AABBCCDDEE01}";
  static final String HIGH="{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}";
  interface Attempt {void run()throws Exception;}
  static void rejects(Attempt action)throws Exception {try{action.run();}catch(ConstraintViolationException expected){return;}throw new AssertionError("Expected rejection");}
  static Table table(Database db,String name,boolean mixed,boolean unique)throws Exception {
    TableBuilder builder=new TableBuilder(name).addColumn(new ColumnBuilder("G",DataType.GUID));
    if(mixed)builder.addColumn(new ColumnBuilder("N",DataType.BIG_INT));
    if(unique)builder.addIndex(new IndexBuilder("UniqueKey").addColumns(mixed?new String[]{"G","N"}:new String[]{"G"}).setUnique());
    return builder.toTable(db);
  }
  public static void main(String[] args)throws Exception {
    if(args.length==2 && args[0].equals("--probe-case")) {
      try(Database db=new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
        IndexCursor cursor=CursorBuilder.createCursor(db.getTable("GuidParents").getIndex("UniqueKey"));
        System.out.println("lower\t"+cursor.findFirstRowByEntry(B.toLowerCase(Locale.ROOT)));
        System.out.println("upper\t"+cursor.findFirstRowByEntry(B.toUpperCase(Locale.ROOT)));
      }
      return;
    }
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table parent=table(db,"GuidParents",false,true),child=table(db,"GuidChildren",false,false),unique=table(db,"GuidUniqueChildren",false,true);
      new RelationshipBuilder(parent,child).addColumns("G","G").setReferentialIntegrity().setName("GuidKeys").toRelationship(db);
      new RelationshipBuilder(parent,unique).addColumns("G","G").setReferentialIntegrity().setName("UniqueGuidKeys").toRelationship(db);
      for(String value:new String[]{ZERO,A,B,HIGH,null,null})parent.addRow(value);
      for(String value:new String[]{ZERO,A.toUpperCase(Locale.ROOT),B,HIGH,null,null})child.addRow(value);
      unique.addRow(A);unique.addRow((Object)null);unique.addRow((Object)null);
      rejects(()->parent.addRow(A.toLowerCase(Locale.ROOT)));rejects(()->child.addRow(OTHER));rejects(()->unique.addRow(A.toLowerCase(Locale.ROOT)));
      Table mixedParent=table(db,"GuidMixedParents",true,true),mixedChild=table(db,"GuidMixedChildren",true,false),mixedUnique=table(db,"GuidMixedUniqueChildren",true,true);
      new RelationshipBuilder(mixedParent,mixedChild).addColumns("G","G").addColumns("N","N").setReferentialIntegrity().setName("GuidMixedKeys").toRelationship(db);
      new RelationshipBuilder(mixedParent,mixedUnique).addColumns("G","G").addColumns("N","N").setReferentialIntegrity().setName("UniqueGuidMixedKeys").toRelationship(db);
      Object[][] pairs={{A,Long.MIN_VALUE},{A,Long.MAX_VALUE},{B,9007199254740993L},{null,9007199254740993L},{HIGH,null},{null,null},{null,null}};
      for(Object[] values:pairs){mixedParent.addRow(values);mixedChild.addRow(values);}
      mixedUnique.addRow(null,9007199254740993L);mixedUnique.addRow(null,null);mixedUnique.addRow(null,null);
      rejects(()->mixedChild.addRow(OTHER,Long.MIN_VALUE));rejects(()->mixedChild.addRow(A,9007199254740993L));rejects(()->mixedChild.addRow(null,Long.MAX_VALUE));rejects(()->mixedChild.addRow(B,null));
      rejects(()->mixedUnique.addRow(null,9007199254740993L));
    }
    System.out.println("All exact GUID/tuple acceptance and rejection assertions passed.");
  }
}
