import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;

// Inert bounded getter comparison: no SQL, expression evaluation or file writes.
public class LargeTableOracle {
  public static void main(String[] args) throws Exception {
    if(args.length!=3) throw new IllegalArgumentException("Expected path, offset and limit");
    int offset=Integer.parseInt(args[1]), limit=Integer.parseInt(args[2]);
    if(offset<0 || offset>1000000 || limit<1 || limit>6000) throw new IllegalArgumentException("Invalid bounded range");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table=db.getTable("LargeRows");
      System.out.println("Meta\t"+table.getRowCount()+"\t"+((TableImpl)table).getOwnedPagesCursor().getUsageMap().getPageCount());
      System.out.println(String.join("\t",table.getColumns().stream().map(Column::getName).toList()));
      Cursor cursor=CursorBuilder.createCursor(table);cursor.moveNextRows(offset);
      for(int i=0;i<limit;i++) {
        Row row=cursor.getNextRow();if(row==null) break;
        StringBuilder line=new StringBuilder("I32:"+row.getInt("ID"));
        for(int column=0;column<6;column++) line.append("\tT").append(HexFormat.of().formatHex(row.getString("P"+column).getBytes(StandardCharsets.UTF_8)));
        System.out.println(line);
      }
    }
  }
}
