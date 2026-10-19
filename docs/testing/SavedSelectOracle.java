import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.file.Path;

/** Read-only independent reconstruction, never query evaluation or execution. */
public class SavedSelectOracle {
  public static void main(String[] args)throws Exception {
    if(args.length<1||args.length>2)throw new IllegalArgumentException("Expected path and optional query name");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      if(args.length==1){for(Query query:db.getQueries())System.out.println(query.getName());return;}
      Query query=db.getQueries().stream().filter(q->q.getName().equals(args[1])).findFirst().orElseThrow();
      if(query.getType()!=Query.Type.SELECT)throw new IllegalArgumentException("Expected SELECT metadata");
      System.out.println(query.toSQLString());
    }
  }
}
