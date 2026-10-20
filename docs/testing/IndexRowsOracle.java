import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
public class IndexRowsOracle {
  public static void main(String[] args) throws Exception {
    if(args.length<4) throw new IllegalArgumentException("Expected path, table, index and column names");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      Table table=db.getTable(args[1]);if(table==null) table=db.getSystemTable(args[1]);
      db.setEvaluateExpressions(false);
      Cursor cursor=CursorBuilder.createCursor(table.getIndex(args[2]));Row row;
      while((row=cursor.getNextRow())!=null) {
        StringBuilder line=new StringBuilder();
        for(int i=3;i<args.length;i++) {
          if(i>3) line.append('\t');
          Object value=row.get(args[i]);
          // Access Byte is unsigned; Java's Byte carrier is signed.
          if(value instanceof Byte) value=Byte.toUnsignedInt((Byte)value);
          if(value instanceof byte[]) value="0x"+java.util.HexFormat.of().formatHex((byte[])value);
          line.append(value==null?"NULL":value);
        }
        System.out.println(line);
      }
    }
  }
}
