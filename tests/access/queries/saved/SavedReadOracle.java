import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.file.Path;

/** Read-only independent SQL rendering; does not bind or evaluate queries. */
public class SavedReadOracle {
  public static void main(String[] args)throws Exception {
    if(args.length!=1&&args.length!=2)throw new IllegalArgumentException("Expected path and optional name");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      if(args.length==1){for(Query query:db.getQueries())System.out.println(query.getName());return;}
      Query query=db.getQueries().stream().filter(q->q.getName().equals(args[1])).findFirst().orElseThrow();
      if(query.getType()!=Query.Type.SELECT&&query.getType()!=Query.Type.UNION&&query.getType()!=Query.Type.CROSS_TAB)throw new IllegalArgumentException("Expected retrieval metadata");
      System.out.println(query.toSQLString());
    }
  }
}
