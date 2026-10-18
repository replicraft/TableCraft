import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
public class TableSchemaOracle {
  public static void main(String[] args) throws Exception {
    try (Database db = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      for (int i=1; i<args.length; i++) {
        Table table=db.getSystemTable(args[i]);
        if(table==null) table=db.getTable(args[i]);
        for(Column c:table.getColumns())
          System.out.printf("%s\t%s\t%d\t%d%n",args[i],c.getName(),Byte.toUnsignedInt(c.getType().getValue()),c.getType()==DataType.BOOLEAN?0:Short.toUnsignedInt(c.getLength()));
      }
    }
  }
}
