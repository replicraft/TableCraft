import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.time.LocalDateTime;

// Original repository-licensed recipe, no native application content/providers.
public class ModernScalarFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table numbers=new TableBuilder("LargeNumbers")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Value",DataType.BIG_INT))
        .addIndex(new IndexBuilder("BigKey").addColumns("Value")).toTable(db);
      Long[] values={Long.MIN_VALUE,Long.MAX_VALUE,9007199254740993L,-9007199254740993L,0L,-1L,null};
      for(int i=0;i<values.length;i++)numbers.addRow(i+1,values[i]);
      new TableBuilder("Calculated").addColumn(new ColumnBuilder("Score",DataType.LONG))
        .addColumn(new ColumnBuilder("Twice",DataType.LONG).setCalculatedInfo("[Score]*2")).toTable(db);
      if(format==Database.FileFormat.V2019) {
        Table dates=new TableBuilder("ExtendedDates")
          .addColumn(new ColumnBuilder("ID",DataType.LONG))
          .addColumn(new ColumnBuilder("Stamp",DataType.EXT_DATE_TIME))
          .toTable(db);
        LocalDateTime[] times={LocalDateTime.of(1,1,1,0,0),LocalDateTime.of(1600,2,29,23,59,59,100),
          LocalDateTime.of(1900,2,28,12,0),LocalDateTime.of(2000,2,29,12,34,56,123456700),
          LocalDateTime.of(9999,12,31,23,59,59,999999900),null};
        for(int i=0;i<times.length;i++)dates.addRow(i+1,times[i]);
      }
    }
  }
}
