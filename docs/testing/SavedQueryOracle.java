import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.file.Path;
import java.util.*;

/** Read-only metadata report; never calls toSQLString or executes any query. */
public class SavedQueryOracle {
  static String escape(String value) {
    StringBuilder text=new StringBuilder();
    value.codePoints().forEach(ch->{switch(ch){
      case '\\'->text.append("\\\\");case '"'->text.append("\\\"");
      case '\t'->text.append("\\t");case '\r'->text.append("\\r");case '\n'->text.append("\\n");
      default->{if(Character.isISOControl(ch))text.append(String.format("\\u{%04x}",ch));else text.appendCodePoint(ch);}
    }});
    return text.toString();
  }
  static String text(Object value){return value==null?"NULL":"\""+escape((String)value)+"\"";}
  static String number(Object value){return value==null?"NULL":value.toString();}
  public static void main(String[] args)throws Exception {
    if(args.length<1||args.length>2)throw new IllegalArgumentException("Expected path and optional query name");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      List<Query> queries=db.getQueries();
      if(args.length==1) {
        for(Query query:queries)System.out.println(query.getName());
        return;
      }
      Query query=queries.stream().filter(q->q.getName().equals(args[1])).findFirst().orElseThrow();
      List<Row> rows=new ArrayList<>();
      for(Row row:db.getSystemTable("MSysQueries"))if(Objects.equals(row.get("ObjectId"),query.getObjectId()))rows.add(row);
      System.out.println("Query\t"+escape(query.getName()));
      System.out.println("Native ID\t"+query.getObjectId());
      System.out.printf("Flags\t0x%08x%n",query.getObjectFlag());
      System.out.println("Fragment rows\t"+rows.size());
      System.out.println("Attribute\tExpression\tFlag\tLvExtra\tName1\tName2\tOrder");
      for(Row row:rows) {
        Byte attribute=row.getByte("Attribute");byte[] order=row.getBytes("Order");
        System.out.printf("%s\t%s\t%s\t%s\t%s\t%s\t%s%n",attribute==null?"NULL":Byte.toUnsignedInt(attribute),
          text(row.get("Expression")),number(row.get("Flag")),number(row.get("LvExtra")),
          text(row.get("Name1")),text(row.get("Name2")),order==null?"NULL":"0x"+HexFormat.of().formatHex(order));
      }
    }
  }
}
