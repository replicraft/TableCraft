import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original read-only raw catalog-payload oracle, with no active-content host. */
public class ObjectPayloadOracle {
  static final String[] FIELDS={"Lv","LvModule","LvExtra"};
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected path and native ID, --all or --list");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);Table catalog=db.getSystemTable("MSysObjects");
      boolean listing=args[1].equals("--list"),all=args[1].equals("--all"),found=false;
      for(Row row:catalog) {
        int id=row.getInt("Id");if(!listing && !all && id!=Integer.parseInt(args[1]))continue;
        found=true;
        if(listing)System.out.printf("%d\t%s\t%d",id,row.getString("Name"),row.getShort("Type"));
        for(String field:FIELDS) {
          boolean declared=catalog.getColumns().stream().anyMatch(column->column.getName().equals(field));
          byte[] bytes=declared?row.getBytes(field):null;
          if(listing)System.out.print("\t"+(!declared?"U":bytes==null?"N":bytes.length));
          else System.out.printf("%s%s\t%s%n",all?id+"\t":"",field,!declared?"U":bytes==null?"N":"X"+HexFormat.of().formatHex(bytes));
        }
        if(listing)System.out.println();else if(!all)break;
      }
      if(!found)throw new IllegalArgumentException("Unknown native object ID");
    }
  }
}
