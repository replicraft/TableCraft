package com.healthmarketscience.jackcess.impl;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Original finite pattern model. Uses the reviewed reference encoder with only
 * its index-only string trimming/prefix step disabled by the portable runner. */
public class GeneralLikeOracle {
  enum Kind { MANY, ONE, DIGIT, LITERAL, CLASS }
  record Atom(Kind kind, String text, boolean negated) {}
  record Spec(String pattern, Atom[] atoms) {}
  static Atom many(){return new Atom(Kind.MANY,"",false);}
  static Atom one(){return new Atom(Kind.ONE,"",false);}
  static Atom digit(){return new Atom(Kind.DIGIT,"",false);}
  static Atom lit(String text){return new Atom(Kind.LITERAL,text,false);}
  static Atom cls(String text,boolean negated){return new Atom(Kind.CLASS,text,negated);}
  static Spec spec(String pattern,Atom... atoms){return new Spec(pattern,atoms);}
  static byte[] key(GeneralLegacyIndexCodes encoder,String text)throws Exception {
    var out=new ByteUtil.ByteStream();encoder.writeNonNullIndexTextValue(text,out,true);return out.toByteArray();
  }
  static boolean inClass(GeneralLegacyIndexCodes encoder,byte[] value,String list)throws Exception {
    boolean found=false;
    for(int at=0;at<list.length();) {
      char low=list.charAt(at++),high=low;
      if(at+1<list.length() && list.charAt(at)=='-'){high=list.charAt(at+1);at+=2;}
      found|=Arrays.compareUnsigned(key(encoder,String.valueOf(low)),value)<=0 &&
             Arrays.compareUnsigned(value,key(encoder,String.valueOf(high)))<=0;
    }
    return found;
  }
  static boolean match(GeneralLegacyIndexCodes encoder,Set<ByteBuffer> elements,String text,int start,Atom[] atoms,int at)throws Exception {
    if(at==atoms.length)return start==text.length();
    Atom atom=atoms[at];
    switch(atom.kind()) {
      case MANY:
        for(int end=start;end<=text.length();end++)if(match(encoder,elements,text,end,atoms,at+1))return true;
        return false;
      case ONE:
        return start<text.length() && match(encoder,elements,text,start+1,atoms,at+1);
      case DIGIT:
        return start<text.length() && text.charAt(start)>='0' && text.charAt(start)<='9' && match(encoder,elements,text,start+1,atoms,at+1);
      case LITERAL:
        byte[] expected=key(encoder,atom.text());
        for(int end=start;end<=text.length();end++)if(Arrays.equals(key(encoder,text.substring(start,end)),expected) && match(encoder,elements,text,end,atoms,at+1))return true;
        return false;
      case CLASS:
        var allowed=new ArrayList<Integer>();boolean excluded=false;
        for(int end=start+1;end<=text.length();end++) {
          byte[] candidate=key(encoder,text.substring(start,end));
          if(!elements.contains(ByteBuffer.wrap(candidate)))continue;
          boolean inside=inClass(encoder,candidate,atom.text());excluded|=inside;
          if(inside!=atom.negated())allowed.add(end);
        }
        if(atom.negated() && excluded)return false;
        for(int end:allowed)if(match(encoder,elements,text,end,atoms,at+1))return true;
        return false;
      default:throw new IllegalStateException();
    }
  }
  public static void main(String[] args)throws Exception {
    int version=Integer.parseInt(args[0]);
    if(version<0 || version>1)throw new IllegalArgumentException("Expected General version 0/1");
    var encoder=version==0?GeneralLegacyIndexCodes.GEN_LEG_INSTANCE:GeneralIndexCodes.GEN_INSTANCE;
    var elements=new HashSet<ByteBuffer>();
    for(int unit=0;unit<=Character.MAX_VALUE;unit++)elements.add(ByteBuffer.wrap(key(encoder,String.valueOf((char)unit))));
    String[] inputs={"","a","A","a ","abc","ss","ß","ae","æ","é","e\u0301","😀","s\u0000s","\u0000","2","２"};
    for(String mode:new String[]{"ansi89","ansi92"}) {
      String many=mode.equals("ansi89")?"*":"%",one=mode.equals("ansi89")?"?":"_",not=mode.equals("ansi89")?"!":"^";
      Spec[] patterns={spec(""),spec("[]"),spec(many,many()),spec(one,one()),spec(one+one,one(),one()),
        spec("ss",lit("ss")),spec("ß",lit("ß")),spec("æ",lit("æ")),spec("ae",lit("ae")),spec("s[]s",lit("ss")),
        spec("[ß]",cls("ß",false)),spec("[a-z]",cls("a-z",false)),spec("[aß]",cls("aß",false)),
        spec("["+not+"ß]"+many,cls("ß",true),many()),spec("["+not+"b]",cls("b",true)),
        spec("["+not+"ß]s",cls("ß",true),lit("s")),spec("a ",lit("a ")),spec(" ",lit(" ")),
        spec("a"+many,lit("a"),many()),spec("a"+one,lit("a"),one()),spec("e\u0301",lit("e\u0301")),
        spec("[é]",cls("é",false)),spec("\u0000",lit("\u0000")),spec("[ ]",cls(" ",false)),
        spec(many+"[a-e]"+many,many(),cls("a-e",false),many()),spec(one+"s",one(),lit("s")),spec("s"+one,lit("s"),one()),
        spec(mode.equals("ansi89")?"#":"[0-9]",mode.equals("ansi89")?digit():cls("0-9",false))};
      for(int text=0;text<inputs.length;text++)for(int pattern=0;pattern<patterns.length;pattern++) {
        Spec spec=patterns[pattern];
        boolean result=match(encoder,elements,inputs[text],0,spec.atoms(),0);
        var hex=HexFormat.of();
        System.out.printf("%s\t%d\t%d\t%s\t%s\t%s%n",mode,text,pattern,
          hex.formatHex(inputs[text].getBytes(StandardCharsets.UTF_8)),hex.formatHex(spec.pattern().getBytes(StandardCharsets.UTF_8)),result?"TRUE":"FALSE");
      }
    }
  }
}
