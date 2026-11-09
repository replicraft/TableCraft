import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import java.nio.file.Path;

/** Original read-only reciprocal-index comparison; never evaluates application code. */
public class ForeignIndexOracle {
  public static void main(String[] args) throws Exception {
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table=db.getTable(args[1]);
      var relationships=db.getRelationships(table);
      var counts=new java.util.HashMap<String,Integer>();
      var output=new StringBuilder();
      for(Index raw:table.getIndexes()) {
        IndexImpl index=(IndexImpl)raw;
        if(!index.isForeignKey())continue;
        var ref=index.getReference();
        var other=index.getReferencedIndex();
        if(ref==null||other==null)throw new IllegalStateException("Missing foreign reference");
        Index parent=ref.isPrimaryTable()?index:other,child=ref.isPrimaryTable()?other:index;
        Relationship match=null;
        for(Relationship relation:relationships) {
          if(!relation.hasReferentialIntegrity() || !relation.getFromTable().getName().equalsIgnoreCase(parent.getTable().getName())
              || !relation.getToTable().getName().equalsIgnoreCase(child.getTable().getName())
              || relation.getFromColumns().size()!=parent.getColumnCount() || relation.getToColumns().size()!=child.getColumnCount())continue;
          boolean same=true;
          for(int i=0;i<parent.getColumnCount();i++) {
            boolean pair=false;
            for(int j=0;j<relation.getFromColumns().size();j++)pair |= relation.getFromColumns().get(j).getName().equalsIgnoreCase(parent.getColumns().get(i).getName())
              && relation.getToColumns().get(j).getName().equalsIgnoreCase(child.getColumns().get(i).getName());
            same &= pair;
          }
          if(!same)continue;
          if(match!=null)throw new IllegalStateException("Ambiguous relationship");
          if(relation.cascadeUpdates()!=ref.isCascadeUpdates() || relation.cascadeDeletes()!=ref.isCascadeDeletes()
              || relation.cascadeNullOnDelete()!=ref.isCascadeNullOnDelete() || !parent.isUnique() || (relation.isOneToOne()&&!child.isUnique()))
            throw new IllegalStateException("Relationship declaration disagrees with index");
          match=relation;
        }
        if(match==null)throw new IllegalStateException("Missing enforced relationship");
        counts.merge(match.getName(),1,Integer::sum);
        int deletes=ref.isCascadeDeletes()?1:ref.isCascadeNullOnDelete()?2:0;
        output.append(table.getName()+"\t"+index.getName()+"\t"+index.getIndexNumber()+"\t"+ref.getTableType()+"\t"+ref.getOtherTablePageNumber()+"\t"+ref.getOtherIndexNumber()+"\t"+(ref.isCascadeUpdates()?1:0)+"\t"+deletes+"\t"+other.getName()+"\n");
      }
      for(Relationship relation:relationships)if(relation.hasReferentialIntegrity()) {
        int expected=(relation.getFromTable().getName().equalsIgnoreCase(table.getName())?1:0)+(relation.getToTable().getName().equalsIgnoreCase(table.getName())?1:0);
        if(counts.getOrDefault(relation.getName(),0)!=expected)throw new IllegalStateException("Incomplete enforced endpoint coverage");
      }
      System.out.print(output);
    }
  }
}
