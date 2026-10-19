import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.Path;
import java.util.TimeZone;

// Original read-only bit oracle. Jackcess's DATE carrier retains the original
// serial bits internally; toDateDouble uses them without a calendar round trip.
public class StoredDateBitsOracle {
  public static void main(String[] args) throws Exception {
    if(args.length != 3) throw new IllegalArgumentException("Expected path, table and column");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);db.setTimeZone(TimeZone.getTimeZone("UTC"));db.setDateTimeType(DateTimeType.DATE);
      Table table=db.getTable(args[1]);ColumnImpl column=(ColumnImpl)table.getColumn(args[2]);
      if(column.getType()!=DataType.SHORT_DATE_TIME)throw new IllegalArgumentException("Expected legacy DateTime column");
      for(Row row:table) {
        Object value=row.get(args[2]);
        if(value==null)System.out.println(args[1]+"\tN");
        else System.out.printf("%s\tY64:%016x%n",args[1],Double.doubleToRawLongBits(column.toDateDouble(value)));
      }
    }
  }
}
