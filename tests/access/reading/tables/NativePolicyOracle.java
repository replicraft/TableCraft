package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Original finite independent weight-model outcomes, never native SQL execution.
 * Inputs are short complete strings, excluding physical prefix/digest behavior. */
public class NativePolicyOracle {
  static GeneralLegacyIndexCodes encoder(ColumnImpl.SortOrder order) {
    if(order.getValue()!=1033 || order.getVariant()!=0) throw new IllegalArgumentException("Unsupported order");
    return switch(order.getVersion()) {case 0->GeneralLegacyIndexCodes.GEN_LEG_INSTANCE;case 1->GeneralIndexCodes.GEN_INSTANCE;default->throw new IllegalArgumentException("Unsupported version");};
  }
  static byte[] key(GeneralLegacyIndexCodes encoder,String value)throws Exception {
    var out=new ByteUtil.ByteStream();encoder.writeNonNullIndexTextValue(value,out,true);return out.toByteArray();
  }
  static int compare(GeneralLegacyIndexCodes encoder,String a,String b)throws Exception {
    if(a==null || b==null) return a==b?0:a==null?-1:1;
    return Integer.signum(Arrays.compareUnsigned(key(encoder,a),key(encoder,b)));
  }
  static String truth(boolean value){return value?"TRUE":"FALSE";}
  static void emit(String name,String expected) {
    System.out.println(name+"\t"+Base64.getEncoder().encodeToString(expected.getBytes(StandardCharsets.UTF_8)));
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2 || !Set.of("database","0","1").contains(args[1])) throw new IllegalArgumentException("Expected path and database/0/1 override");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table=db.getTable("PolicyProbe");
      var defaults=((DatabaseImpl)db).getDefaultSortOrder();
      var legacy=((ColumnImpl)table.getColumn("LegacyText")).getTextSortOrder();
      var modern=((ColumnImpl)table.getColumn("ModernText")).getTextSortOrder();
      var root=encoder(defaults);var left=encoder(legacy);var right=encoder(modern);
      if(!args[1].equals("database")) {root=args[1].equals("0")?GeneralLegacyIndexCodes.GEN_LEG_INSTANCE:GeneralIndexCodes.GEN_INSTANCE;left=root;right=root;}
      List<Row> rows=new ArrayList<>();for(Row row:table)rows.add(row);
      Row first=rows.stream().filter(r->r.getInt("ID")==1).findFirst().orElseThrow();
      int defaultOrder=compare(root,"\u0080","\u0301");
      emit("compare","ID\tL\tM\tD\tModuleMode\tDatabaseMode\n1\t"+truth(compare(left,first.getString("LegacyText"),"\u0301")<0)+"\t"+truth(compare(right,first.getString("ModernText"),"\u0301")<0)+"\t"+truth(defaultOrder<0)+"\t"+defaultOrder+"\t"+defaultOrder+"\n");
      for(String field:new String[]{"LegacyText","ModernText"}) {
        var weights=field.equals("LegacyText")?left:right;
        var sorted=new ArrayList<>(rows);
        sorted.sort((a,b)->{try {int c=compare(weights,a.getString(field),b.getString(field));return c==0?Integer.compare(a.getInt("ID"),b.getInt("ID")):c;}catch(Exception e){throw new RuntimeException(e);}});
        var order=new StringBuilder("ID\n");for(Row row:sorted)order.append(row.getInt("ID")).append('\n');
        emit("order"+field,order.toString());
        var filter=new StringBuilder("ID\n");
        for(Row row:rows)if(row.getString(field)!=null && compare(weights,row.getString(field),"\u0301")<0)filter.append(row.getInt("ID")).append('\n');
        emit("filter"+field,filter.toString());
      }
      List<List<Row>> groups=new ArrayList<>();
      for(Row row:rows) {
        List<Row> found=null;
        for(List<Row> group:groups)if(compare(left,group.get(0).getString("LegacyText"),row.getString("LegacyText"))==0){found=group;break;}
        if(found==null){found=new ArrayList<>();groups.add(found);}found.add(row);
      }
      var grouped=new StringBuilder("ID\tN\n");for(List<Row> group:groups)grouped.append(group.get(0).getInt("ID")).append('\t').append(group.size()).append('\n');
      emit("group",grouped.toString());
      if(compare(right,"\u0301","\u0080")<=0)emit("like","M\n"+truth(compare(right,first.getString("ModernText"),"\u0301")>=0 && compare(right,first.getString("ModernText"),"\u0080")<=0)+"\n");
    }
  }
}
