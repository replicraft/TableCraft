import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;

/** Original inert exact Currency/integer relationship recipe. */
public class CurrencyRelationshipFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action)throws Exception {
    try {action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static BigDecimal money(long scaled){return BigDecimal.valueOf(scaled,4);}
  static Table table(Database db,String name,boolean unique,boolean mixed)throws Exception {
    TableBuilder builder=new TableBuilder(name).addColumn(new ColumnBuilder("Amount",DataType.MONEY));
    if(mixed)builder.addColumn(new ColumnBuilder("Band",DataType.LONG));
    if(unique)builder.addIndex(new IndexBuilder("UniqueKey")
      .addColumns(mixed?new String[]{"Amount","Band"}:new String[]{"Amount"}).setUnique());
    return builder.toTable(db);
  }
  static void relation(Database db,Table parent,Table child,String name,boolean mixed)throws Exception {
    RelationshipBuilder builder=new RelationshipBuilder(parent,child).addColumns("Amount","Amount");
    if(mixed)builder.addColumns("Band","Band");
    builder.setReferentialIntegrity().setName(name).toRelationship(db);
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table parent=table(db,"MoneyParents",true,false),child=table(db,"MoneyChildren",false,false),
        unique=table(db,"MoneyUniqueChildren",true,false);
      relation(db,parent,child,"MoneyKeys",false);relation(db,parent,unique,"UniqueMoneyKeys",false);
      long[] values={Long.MIN_VALUE,Long.MIN_VALUE+1,-9007199254740993L,-1L,0L,1L,
        9007199254740992L,9007199254740993L,Long.MAX_VALUE-1,Long.MAX_VALUE};
      for(long value:values){parent.addRow(money(value));child.addRow(money(value));}
      for(int i=0;i<2;i++){parent.addRow((Object)null);child.addRow((Object)null);}
      unique.addRow(money(Long.MIN_VALUE));unique.addRow(money(Long.MIN_VALUE+1));
      unique.addRow((Object)null);unique.addRow((Object)null);
      rejects(()->child.addRow(money(2)));rejects(()->unique.addRow(money(Long.MIN_VALUE)));

      Table mixedParent=table(db,"MoneyMixedParents",true,true),mixedChild=table(db,"MoneyMixedChildren",false,true),
        mixedUnique=table(db,"MoneyMixedUniqueChildren",true,true);
      relation(db,mixedParent,mixedChild,"MoneyMixedKeys",true);
      relation(db,mixedParent,mixedUnique,"UniqueMoneyMixedKeys",true);
      Object[][] pairs={{money(Long.MIN_VALUE),Integer.MIN_VALUE},{money(Long.MAX_VALUE),Integer.MAX_VALUE},
        {money(9007199254740992L),7},{money(9007199254740993L),8},
        {null,Integer.MAX_VALUE},{money(Long.MAX_VALUE),null},{null,null},{null,null}};
      for(Object[] pair:pairs){mixedParent.addRow(pair);mixedChild.addRow(pair);}
      mixedUnique.addRow((Object)null,Integer.MAX_VALUE);mixedUnique.addRow(money(Long.MAX_VALUE),null);
      mixedUnique.addRow(null,null);mixedUnique.addRow(null,null);
      rejects(()->mixedChild.addRow(money(Long.MIN_VALUE),Integer.MAX_VALUE));
      rejects(()->mixedChild.addRow(money(9007199254740993L),7));
      rejects(()->mixedChild.addRow(null,Integer.MIN_VALUE));
      rejects(()->mixedChild.addRow(money(Long.MIN_VALUE),null));
      rejects(()->mixedUnique.addRow(null,Integer.MAX_VALUE));
    }
    System.out.println("All exact Currency relationship assertions passed.");
  }
}
