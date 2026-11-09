import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;
import java.util.*;

/** Original inert mixed/all-GUID composite recipe, fresh output paths only. */
public class GuidCompositeIndexFixture {
  interface Attempt {void run()throws Exception;}
  static void rejects(Attempt action)throws Exception {
    try{action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static BigDecimal money(long value){return BigDecimal.valueOf(value,4);}
  static String guid(long high,long low){return "{"+new UUID(high,low).toString().toUpperCase(Locale.ROOT)+"}";}
  static TableBuilder pair(String name){return new TableBuilder(name).addColumn(new ColumnBuilder("G1",DataType.GUID))
    .addColumn(new ColumnBuilder("Amount",DataType.MONEY)).addColumn(new ColumnBuilder("G2",DataType.GUID));}
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    long[] bits={0L,-1L,Long.MIN_VALUE,Long.MAX_VALUE,1L,1L<<32,1L<<56,0x0001000000000000L,
      0x0102030405060708L,0x8080808080808080L,2L,1L<<16,1L<<48,17L,0x0123456789ABCDEFL,0x6B29FC40CA471067L};
    String[] keys=new String[bits.length];for(int i=0;i<keys.length;i++)keys[i]=guid(bits[i],bits[i]);
    long[] longs={Long.MIN_VALUE,Long.MIN_VALUE+1,-9007199254740993L,-1L,0L,1L,9007199254740992L,9007199254740993L,Long.MAX_VALUE-1,Long.MAX_VALUE};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table mixed=new TableBuilder("GuidMixedIndexed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("G1",DataType.GUID)).addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addColumn(new ColumnBuilder("Large",DataType.BIG_INT)).addColumn(new ColumnBuilder("G2",DataType.GUID))
        .addColumn(new ColumnBuilder("Narrow",DataType.INT))
        .addIndex(new IndexBuilder("FiveAsc").addColumns("G1","Amount","Large","G2","Narrow"))
        .addIndex(new IndexBuilder("FiveDesc").addColumns(false,"G1","Amount","Large","G2","Narrow"))
        .addIndex(new IndexBuilder("MixedDirections").addColumns(false,"G2").addColumns("Amount","G1")
          .addColumns(false,"Narrow","Large")).toTable(db);
      for(int i=0;i<1000;i++)mixed.addRow(i,i%11==0?null:keys[(i*7)%keys.length],
        i%13==0?null:money(longs[(i*3)%longs.length]),i%17==0?null:longs[(i*7)%longs.length],
        i%19==0?null:keys[(i*11+5)%keys.length],i%23==0?null:(i%3==0?-32768:i%3==1?32767:0));
      TableBuilder builder=new TableBuilder("GuidTenIndexed").addColumn(new ColumnBuilder("ID",DataType.LONG));
      IndexBuilder asc=new IndexBuilder("TenAsc"),desc=new IndexBuilder("TenDesc"),both=new IndexBuilder("TenMixed");
      for(int k=0;k<10;k++){builder.addColumn(new ColumnBuilder("K"+k,DataType.GUID));asc.addColumns("K"+k);desc.addColumns(false,"K"+k);both.addColumns(k%2==0,"K"+k);}
      Table ten=builder.addIndex(asc).addIndex(desc).addIndex(both).toTable(db);
      for(int i=0;i<500;i++) {
        Object[] row=new Object[11];row[0]=i;
        for(int k=0;k<10;k++)row[k+1]=i%17==0||i%(k+5)==0?null:keys[(i*(k*2+1)+k)%keys.length];
        ten.addRow(row);
      }
      Table unique=pair("GuidTupleUnique").addIndex(new IndexBuilder("UniqueTuple")
        .addColumns(false,"G2").addColumns("G1").addColumns(false,"Amount").setUnique()).toTable(db);
      Object[][] pairs={{keys[0],money(Long.MIN_VALUE),keys[1]},{keys[1],money(Long.MAX_VALUE),keys[0]},
        {keys[0],money(9007199254740992L),keys[0]},{keys[0],money(9007199254740993L),keys[0]},
        {null,money(0),keys[0]},{keys[0],null,keys[0]},{keys[0],money(0),null},{null,null,null},{null,null,null}};
      for(Object[] row:pairs)unique.addRow(row);
      rejects(()->unique.addRow(null,money(0),keys[0]));rejects(()->unique.addRow(keys[0],money(9007199254740993L),keys[0]));
      Table primary=pair("GuidTuplePrimary").addIndex(new IndexBuilder("PrimaryTuple")
        .addColumns(false,"G1").addColumns("Amount").addColumns(false,"G2").setPrimaryKey()).toTable(db);
      primary.addRow(keys[0],money(Long.MIN_VALUE),keys[1]);primary.addRow(keys[1],money(Long.MAX_VALUE),keys[0]);primary.addRow(keys[0],money(0),keys[0]);
      rejects(()->primary.addRow(null,money(0),keys[0]));rejects(()->primary.addRow(keys[0],null,keys[0]));
      rejects(()->primary.addRow(keys[0],money(0),null));rejects(()->primary.addRow(null,null,null));
      rejects(()->primary.addRow(keys[0],money(0),keys[0]));
      new TableBuilder("EmptyGuidTuple").addColumn(new ColumnBuilder("G",DataType.GUID)).addColumn(new ColumnBuilder("N",DataType.BIG_INT))
        .addIndex(new IndexBuilder("EmptyTuple").addColumns("G").addColumns(false,"N")).toTable(db);
      new TableBuilder("GuidTextTuple").addColumn(new ColumnBuilder("G",DataType.GUID)).addColumn(new ColumnBuilder("T",DataType.TEXT).setLengthInUnits(10))
        .addIndex(new IndexBuilder("GuidText").addColumns("G","T")).toTable(db);
    }
    System.out.println("GUID-bearing tuple unique/primary acceptance and rejection assertions passed.");
  }
}
