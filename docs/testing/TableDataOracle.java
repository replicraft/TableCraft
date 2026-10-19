import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import java.math.BigDecimal;
import java.nio.ByteOrder;
import java.util.TimeZone;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
public class TableDataOracle {
  private static String encode(Object value, DataType type) {
    if(value==null) return "N";
    return switch(type) {
      case BOOLEAN -> "B"+((Boolean)value ? 1 : 0);
      case BYTE -> "U"+Byte.toUnsignedInt((Byte)value);
      case INT -> "I16:"+value;
      case LONG -> "I32:"+value;
      case BIG_INT -> "I64:"+value;
      case EXT_DATE_TIME -> {
        var date=(java.time.LocalDateTime)value;
        yield String.format("Z%04d-%02d-%02d %02d:%02d:%02d.%07d",date.getYear(),date.getMonthValue(),date.getDayOfMonth(),date.getHour(),date.getMinute(),date.getSecond(),date.getNano()/100);
      }
      case GUID -> "G"+value.toString().replace("{", "").replace("}", "").replace("-", "").toLowerCase(java.util.Locale.ROOT);
      case NUMERIC -> "Q"+((BigDecimal)value).unscaledValue()+":"+((BigDecimal)value).scale();
      case MONEY -> "C"+((BigDecimal)value).movePointRight(4).longValueExact();
      case FLOAT -> String.format("F%08x",Float.floatToRawIntBits((Float)value));
      case DOUBLE -> String.format("D%016x",Double.doubleToRawLongBits((Double)value));
      case SHORT_DATE_TIME -> {
        var date=(java.time.LocalDateTime)value;
        yield String.format("Y%04d-%02d-%02d %02d:%02d:%02d.%09d",date.getYear(),date.getMonthValue(),date.getDayOfMonth(),date.getHour(),date.getMinute(),date.getSecond(),date.getNano());
      }
      case TEXT, MEMO -> "T"+HexFormat.of().formatHex(((String)value).getBytes(StandardCharsets.UTF_8));
      case BINARY, OLE -> "X"+HexFormat.of().formatHex((byte[])value);
      default -> throw new IllegalArgumentException("Unsupported comparison carrier");
    };
  }
  private static void rows(String label, Table table, Cursor cursor, boolean dateBits) throws Exception {
    var names=table.getColumns().stream().map(Column::getName).toList();Row row;
    while((row=cursor.getNextRow(names))!=null) {
      StringBuilder line=new StringBuilder(label);
      for(Column c:table.getColumns()) {
        Object value=row.get(c.getName());line.append('\t');
        if(dateBits && c.getType()==DataType.SHORT_DATE_TIME && value!=null) {
          if(c.isCalculated()) throw new IllegalArgumentException("Use StoredDateBitsOracle for cached DATE fields");
          var bytes=((ColumnImpl)c).write(value,0,ByteOrder.LITTLE_ENDIAN);
          if(bytes.limit()!=Long.BYTES)throw new IllegalArgumentException("Unexpected DATE width");
          line.append(String.format("Y64:%016x",bytes.getLong(0)));
        } else line.append(encode(value,c.getType()));
      }
      System.out.println(line);
    }
  }
  public static void main(String[] args) throws Exception {
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      boolean dateBits=args.length>1 && args[1].equals("--date-bits");int first=dateBits?2:1;
      if(dateBits) {db.setDateTimeType(DateTimeType.DATE);db.setTimeZone(TimeZone.getTimeZone("UTC"));}
      if(args.length==first+3 && args[first].equals("--index")) {
        Table table=db.getTable(args[first+1]);
        rows(args[first+1],table,CursorBuilder.createCursor(table.getIndex(args[first+2])),dateBits);
        return;
      }
      for(int i=first;i<args.length;i++) {
        Table table=db.getSystemTable(args[i]);if(table==null) table=db.getTable(args[i]);
        rows(args[i],table,CursorBuilder.createCursor(table),dateBits);
      }
    }
  }
}
