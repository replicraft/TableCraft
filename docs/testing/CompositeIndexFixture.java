import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;

/** Original inert multi-column numeric index fixtures, fresh outputs only. */
public class CompositeIndexFixture {
  interface Attempt {void run()throws Exception;}
  static void rejects(Attempt action)throws Exception {
    try{action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static BigDecimal money(long value){return BigDecimal.valueOf(value,4);}
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    Long[] longs={Long.MIN_VALUE,Long.MIN_VALUE+1,-9007199254740993L,-1L,0L,1L,9007199254740992L,9007199254740993L,Long.MAX_VALUE-1,Long.MAX_VALUE};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table mixed=new TableBuilder("MixedIndexed")
        .addColumn(new ColumnBuilder("ID",DataType.LONG)).addColumn(new ColumnBuilder("Tiny",DataType.BYTE))
        .addColumn(new ColumnBuilder("Small",DataType.INT)).addColumn(new ColumnBuilder("Normal",DataType.LONG))
        .addColumn(new ColumnBuilder("Large",DataType.BIG_INT)).addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addIndex(new IndexBuilder("FiveAsc").addColumns("Tiny","Small","Normal","Large","Amount"))
        .addIndex(new IndexBuilder("FiveDesc").addColumns(false,"Tiny","Small","Normal","Large","Amount"))
        .addIndex(new IndexBuilder("FiveMixed").addColumns(false,"Amount").addColumns("Tiny")
          .addColumns(false,"Large").addColumns("Small").addColumns(false,"Normal")).toTable(db);
      for(int i=0;i<1000;i++)mixed.addRow(i,i%11==0?null:(i*37)%256,i%13==0?null:(i%3==0?-32768:i%3==1?32767:0),
        i%17==0?null:(i%3==0?Integer.MIN_VALUE:i%3==1?Integer.MAX_VALUE:0),
        i%19==0?null:longs[(i*7)%longs.length],i%23==0?null:money(longs[(i*3)%longs.length]));
      TableBuilder tenBuilder=new TableBuilder("TenIndexed").addColumn(new ColumnBuilder("ID",DataType.LONG));
      IndexBuilder tenAsc=new IndexBuilder("TenAsc"),tenDesc=new IndexBuilder("TenDesc"),tenMixed=new IndexBuilder("TenMixed");
      for(int i=0;i<10;i++) {
        tenBuilder.addColumn(new ColumnBuilder("K"+i,i%2==0?DataType.BIG_INT:DataType.MONEY));
        tenAsc.addColumns("K"+i);tenDesc.addColumns(false,"K"+i);tenMixed.addColumns(i%2==0,"K"+i);
      }
      Table ten=tenBuilder.addIndex(tenAsc).addIndex(tenDesc).addIndex(tenMixed).toTable(db);
      for(int i=0;i<500;i++) {
        Object[] row=new Object[11];row[0]=i;
        for(int k=0;k<10;k++) {
          if(i%17==0||i%(k+5)==0)row[k+1]=null;
          else {long value=longs[(i*(k*2+1)+k)%longs.length];row[k+1]=k%2==0?value:money(value);}
        }
        ten.addRow(row);
      }
      Table unique=new TableBuilder("PairUnique").addColumn(new ColumnBuilder("A",DataType.MONEY))
        .addColumn(new ColumnBuilder("B",DataType.BIG_INT))
        .addIndex(new IndexBuilder("UniquePair").addColumns("A").addColumns(false,"B").setUnique()).toTable(db);
      Object[][] pairs={{money(Long.MIN_VALUE),Long.MAX_VALUE},{money(Long.MAX_VALUE),Long.MIN_VALUE},
        {money(-1),1L},{money(0),0L},{money(9007199254740992L),7L},{money(9007199254740993L),7L},
        {null,1L},{money(Long.MAX_VALUE),null},{null,null},{null,null}};
      for(Object[] pair:pairs)unique.addRow(pair);
      rejects(()->unique.addRow(money(9007199254740993L),7L));rejects(()->unique.addRow(null,1L));
      Table primary=new TableBuilder("PairPrimary").addColumn(new ColumnBuilder("A",DataType.MONEY))
        .addColumn(new ColumnBuilder("B",DataType.BIG_INT))
        .addIndex(new IndexBuilder("PrimaryPair").addColumns(false,"A").addColumns("B").setPrimaryKey()).toTable(db);
      primary.addRow(money(-1),Long.MAX_VALUE);primary.addRow(money(0),0L);primary.addRow(money(Long.MAX_VALUE),Long.MIN_VALUE);
      rejects(()->primary.addRow(null,0L));rejects(()->primary.addRow(money(0),null));
      rejects(()->primary.addRow(null,null));rejects(()->primary.addRow(money(0),0L));
      new TableBuilder("EmptyPair").addColumn(new ColumnBuilder("A",DataType.MONEY))
        .addColumn(new ColumnBuilder("B",DataType.BIG_INT))
        .addIndex(new IndexBuilder("EmptyPairKey").addColumns("A").addColumns(false,"B")).toTable(db);
      new TableBuilder("SkipPair").addColumn(new ColumnBuilder("A",DataType.LONG)).addColumn(new ColumnBuilder("B",DataType.LONG))
        .addIndex(new IndexBuilder("SkipPairKey").addColumns("A","B").setIgnoreNulls()).toTable(db);
      new TableBuilder("TextPair").addColumn(new ColumnBuilder("A",DataType.LONG)).addColumn(new ColumnBuilder("B",DataType.TEXT).setLengthInUnits(10))
        .addIndex(new IndexBuilder("TextPairKey").addColumns("A","B")).toTable(db);
    }
    System.out.println("Composite exact-key and NULL constraint assertions passed.");
  }
}
