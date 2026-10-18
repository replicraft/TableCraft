import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import java.math.BigDecimal;
public class TableDataOracle {
  private static String encode(Object value, DataType type) {
    if(value==null) return "N";
    return switch(type) {
      case BOOLEAN -> "B"+((Boolean)value ? 1 : 0);
      case BYTE -> "U"+Byte.toUnsignedInt((Byte)value);
      case INT -> "I16:"+value;
      case LONG -> "I32:"+value;
      case MONEY -> "C"+((BigDecimal)value).movePointRight(4).longValueExact();
      case FLOAT -> String.format("F%08x",Float.floatToRawIntBits((Float)value));
      case DOUBLE -> String.format("D%016x",Double.doubleToRawLongBits((Double)value));
      case SHORT_DATE_TIME -> {
        var date=(java.time.LocalDateTime)value;
        yield String.format("Y%04d-%02d-%02d %02d:%02d:%02d.%09d",date.getYear(),date.getMonthValue(),date.getDayOfMonth(),date.getHour(),date.getMinute(),date.getSecond(),date.getNano());
      }
      case TEXT, MEMO -> "T"+HexFormat.of().formatHex(((String)value).getBytes(StandardCharsets.UTF_8));
      case BINARY -> "X"+HexFormat.of().formatHex((byte[])value);
      default -> throw new IllegalArgumentException("Unsupported comparison carrier");
    };
  }
  public static void main(String[] args) throws Exception {
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      for(int i=1;i<args.length;i++) {
        Table table=db.getSystemTable(args[i]);if(table==null) table=db.getTable(args[i]);
        var names=table.getColumns().stream().map(Column::getName).toList();
        Cursor cursor=CursorBuilder.createCursor(table);Row row;
        while((row=cursor.getNextRow(names))!=null) {
          StringBuilder line=new StringBuilder(args[i]);
          for(Column c:table.getColumns()) line.append('\t').append(encode(row.get(c.getName()),c.getType()));
          System.out.println(line);
        }
      }
    }
  }
}
