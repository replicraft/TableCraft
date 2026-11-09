import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;

/** Original inert exact-Currency index recipe; fresh output paths only. */
public class CurrencyIndexFixture {
  private static BigDecimal money(long scaled) { return BigDecimal.valueOf(scaled, 4); }
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    long[] keys={Long.MIN_VALUE,Long.MIN_VALUE+1,-9007199254740993L,-10001L,-1L,0L,1L,10001L,
      9007199254740992L,9007199254740993L,Long.MAX_VALUE-1,Long.MAX_VALUE};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table table=new TableBuilder("MoneyIndexed")
        .addColumn(new ColumnBuilder("ID",DataType.LONG)).addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("MoneyAsc").addColumns("Amount"))
        .addIndex(new IndexBuilder("MoneyDesc").addColumns(false,"Amount")).toTable(db);
      for(int i=0;i<700;i++)table.addRow(i,i%11==0?null:money(keys[(i*7)%keys.length]));
      Table unique=new TableBuilder("MoneyUnique").addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addIndex(new IndexBuilder("MoneyUniqueKey").addColumns("Amount").setUnique()).toTable(db);
      for(long key:keys)unique.addRow(money(key));
      unique.addRow((Object)null);unique.addRow((Object)null);
      try {unique.addRow(money(Long.MAX_VALUE));throw new AssertionError("Duplicate accepted");}
      catch(ConstraintViolationException expected){}
      Table primary=new TableBuilder("MoneyPrimary").addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addIndex(new IndexBuilder("MoneyPrimaryKey").addColumns("Amount").setPrimaryKey()).toTable(db);
      for(long key:keys)primary.addRow(money(key));
      try {primary.addRow((Object)null);throw new AssertionError("NULL primary accepted");}
      catch(ConstraintViolationException expected){}
      try {primary.addRow(money(1));throw new AssertionError("Duplicate primary accepted");}
      catch(ConstraintViolationException expected){}
      new TableBuilder("MoneyEmpty").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addIndex(new IndexBuilder("EmptyMoneyKey").addColumns("Amount")).toTable(db);
    }
  }
}
