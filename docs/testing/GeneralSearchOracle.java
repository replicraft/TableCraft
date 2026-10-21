package com.healthmarketscience.jackcess.impl;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HexFormat;

/** Original bounded span model using the independently maintained key encoder.
 * No native SQL executes; all values are shorter than the native key prefix. */
public class GeneralSearchOracle {
  static String hex(String value) {
    return HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
  }
  static byte[] key(GeneralLegacyIndexCodes encoder, String value) throws Exception {
    var output = new ByteUtil.ByteStream();
    encoder.writeNonNullIndexTextValue(value, output, true);
    return output.toByteArray();
  }
  static int[] span(GeneralLegacyIndexCodes encoder, String text, String needle, int bound, boolean reverse) throws Exception {
    byte[] expected = key(encoder, needle);
    int[] starts = text.codePoints().toArray();
    int[] offsets = new int[starts.length + 1];
    for(int i = 0; i < starts.length; i++) offsets[i + 1] = offsets[i] + Character.charCount(starts[i]);
    for(int number = 0; number < offsets.length; number++) {
      int index = reverse ? offsets.length - 1 - number : number;
      int start = offsets[index];
      if(reverse ? start >= bound : start < bound) continue;
      for(int end : offsets) {
        if(end <= start || (reverse && end > bound)) continue;
        if(Arrays.equals(expected, key(encoder, text.substring(start,end)))) return new int[]{start,end};
      }
    }
    return null;
  }
  static int position(GeneralLegacyIndexCodes encoder, String text, String needle, int start, boolean reverse) throws Exception {
    if(text.isEmpty() || start > text.length()) return 0;
    if(needle.isEmpty()) return start;
    int[] match = span(encoder,text,needle,reverse ? start : start - 1,reverse);
    return match == null ? 0 : match[0] + 1;
  }
  static String replace(GeneralLegacyIndexCodes encoder, String text, String needle, int count) throws Exception {
    if(needle.isEmpty()) return text;
    var output = new StringBuilder();
    while(!text.isEmpty() && count != 0) {
      int[] match = span(encoder,text,needle,0,false);
      if(match == null) break;
      output.append(text,0,match[0]).append('!');
      text = text.substring(match[1]);
      if(count > 0) count--;
    }
    return output.append(text).toString();
  }
  public static void main(String[] args) throws Exception {
    if(args.length != 1 || !(args[0].equals("0") || args[0].equals("1"))) throw new IllegalArgumentException("Expected General version 0/1");
    var encoder = args[0].equals("0") ? GeneralLegacyIndexCodes.GEN_LEG_INSTANCE : GeneralIndexCodes.GEN_INSTANCE;
    String[][] cases = {{"Straße STRASSE","ss"},{"SS ß","ß"},{"😀éXé","É"},{"e\u0301É","é"},
                        {"\u0000a","a"},{"a\u0000","\u0000"},{"a ","a "},{"カーカ","カ"},
                        {"ababa","aba"},{"", ""},{"abc",""},{"","x"}};
    for(int id = 0; id < cases.length; id++) {
      String text = cases[id][0], needle = cases[id][1];
      for(boolean reverse : new boolean[]{false,true}) {
        for(int start = 1; start <= text.length() + 1; start++) {
          System.out.printf("%s\t%d\t%s\t%s\t%d\t%d%n",reverse ? "R" : "F",id,hex(text),hex(needle),start,position(encoder,text,needle,start,reverse));
        }
      }
      for(int count : new int[]{-1,1}) {
        System.out.printf("P\t%d\t%s\t%s\t%d\t%s%n",id,hex(text),hex(needle),count,hex(replace(encoder,text,needle,count)));
      }
    }
  }
}
