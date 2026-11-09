package com.healthmarketscience.jackcess.impl;

import java.util.HexFormat;

/** Original engineering exporter of reviewed Apache-2.0 weight data, not runtime code. */
public class ExportGeneralWeights {
    private static String bytes(byte[] value) {
        return value == null || value.length == 0 ? "-" : HexFormat.of().formatHex(value);
    }
    public static void main(String[] args) throws Exception {
        if (args.length == 1 && args[0].equals("--vowels")) {
            var method = GeneralLegacyIndexCodes.class.getDeclaredMethod("prolongedSoundBytes", byte[].class);
            method.setAccessible(true);
            byte[] vowels = new byte[256];
            for (int value = 0; value < vowels.length; value++) {
                byte[] mapped = (byte[]) method.invoke(null, (Object) new byte[]{0x7f, (byte) value});
                vowels[value] = mapped[1];
            }
            System.out.println(HexFormat.of().formatHex(vowels));
            return;
        }
        for (var encoder : new GeneralLegacyIndexCodes[]{GeneralLegacyIndexCodes.GEN_LEG_INSTANCE, GeneralIndexCodes.GEN_INSTANCE}) {
            for (int code = 0; code <= Character.MAX_VALUE; code++) {
                char character = (char) code;
                var handler = encoder.getCharHandler(character);
                System.out.printf("%04x\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s%n",
                        code, handler.getType().name(), handler.getNumCharUnits(), handler.getNumPrimaryUnits(),
                        handler.getExtraPlaceholder() & 0xff, handler.getExtraByteModifier() & 0xff,
                        handler.getCrazyFlag() & 0xff, handler.isProlongedSound() ? 1 : 0,
                        bytes(handler.getInlineBytes(character)), bytes(handler.getExtraBytes()),
                        bytes(handler.getUnprintableBytes()), bytes(encoder.getTertiaryWeights(character)));
            }
        }
    }
}
