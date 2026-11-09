import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.nio.file.Path;
import java.nio.ByteOrder;
import java.util.TimeZone;

// Original read-only bit oracle. Jackcess's DATE carrier retains the original
// serial bits internally; serializing that carrier preserves them. toDateDouble
// instead uses a millisecond calendar round trip and can change valid bits.
public class StoredDateBitsOracle {
  private static final int CALC_PREFIX_BYTES=16;
  private static final int CALC_PAYLOAD_OFFSET=CALC_PREFIX_BYTES+Integer.BYTES;
  private static final int CALC_TRAILER_BYTES=3;
  private static long serialBits(ColumnImpl column,Object value) throws Exception {
    var bytes=column.write(value,0,ByteOrder.LITTLE_ENDIAN);
    int offset=column.isCalculated()?CALC_PAYLOAD_OFFSET:0;
    int trailer=column.isCalculated()?CALC_TRAILER_BYTES:0;
    if(bytes.limit()!=offset+Long.BYTES+trailer)throw new IllegalArgumentException("Unexpected DATE serialization width");
    if(column.isCalculated()) {
      for(int i=0;i<CALC_PREFIX_BYTES;i++)if(bytes.get(i)!=0)throw new IllegalArgumentException("Unknown calculated prefix");
      if(bytes.getInt(CALC_PREFIX_BYTES)!=Long.BYTES)throw new IllegalArgumentException("Unknown calculated DATE length");
      for(int i=offset+Long.BYTES;i<bytes.limit();i++)if(bytes.get(i)!=0)throw new IllegalArgumentException("Unknown calculated trailer");
    }
    return bytes.getLong(offset);
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 3) throw new IllegalArgumentException("Expected path, table and column");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);db.setTimeZone(TimeZone.getTimeZone("UTC"));db.setDateTimeType(DateTimeType.DATE);
      Table table=db.getTable(args[1]);ColumnImpl column=(ColumnImpl)table.getColumn(args[2]);
      if(column.getType()!=DataType.SHORT_DATE_TIME)throw new IllegalArgumentException("Expected legacy DateTime column");
      for(Row row:table) {
        Object value=row.get(args[2]);
        if(value==null)System.out.println(args[1]+"\tN");
        else System.out.printf("%s\tY64:%016x%n",args[1],serialBits(column,value));
      }
    }
  }
}
