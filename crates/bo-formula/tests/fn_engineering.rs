//! Engineering functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (definitions of the base conversions, bit operations,
//! DELTA, GESTEP, ERF, ERFC, BESSEL*, COMPLEX, IM*, CONVERT). Expected values are worked out
//! by hand from the definitions (two's complement in 10 digits, complex arithmetic, exact unit
//! definitions) or taken from the published tables of Abramowitz & Stegun, *Handbook of
//! Mathematical Functions* (Bessel functions Table 9.1 / 9.8, error function Table 7.1),
//! re-checked with SciPy / Python `math`.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[("A1", "3+4i"), ("A2", "5-3i"), ("A3", "")];

#[test]
fn number_bases() {
    check(
        DATA,
        &[
            ("=BIN2DEC(\"1100100\")", N(100.0), "hand: 64+32+4"),
            (
                "=BIN2DEC(1100100)",
                N(100.0),
                "hand: a number argument is read as digits",
            ),
            (
                "=BIN2DEC(\"1111111111\")",
                N(-1.0),
                "hand: 10-bit two's complement",
            ),
            ("=BIN2DEC(\"2\")", E("#NUM!"), "ECMA-376 §18.17.7 BIN2DEC"),
            (
                "=BIN2DEC(\"11111111111\")",
                E("#NUM!"),
                "ECMA-376 §18.17.7 BIN2DEC",
            ),
            (
                "=BIN2HEX(\"11111011\",4)",
                T("00FB"),
                "hand: 251 = 0xFB padded to 4",
            ),
            ("=BIN2HEX(\"1110\")", T("E"), "hand: 14"),
            (
                "=BIN2HEX(\"1111111111\")",
                T("FFFFFFFFFF"),
                "hand: -1 in 10 hex digits",
            ),
            (
                "=BIN2HEX(\"11111011\",1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 BIN2HEX",
            ),
            ("=BIN2OCT(\"1001\",3)", T("011"), "hand: 9 = 0o11"),
            ("=BIN2OCT(\"1100100\")", T("144"), "hand: 100 = 0o144"),
            (
                "=BIN2OCT(\"1111111111\")",
                T("7777777777"),
                "hand: -1 in 10 octal digits",
            ),
            ("=DEC2BIN(9,4)", T("1001"), "hand: 8+1"),
            ("=DEC2BIN(9.9)", T("1001"), "hand: truncated to 9"),
            ("=DEC2BIN(-100)", T("1110011100"), "hand: 1024-100 = 924"),
            ("=DEC2BIN(512)", E("#NUM!"), "ECMA-376 §18.17.7 DEC2BIN"),
            ("=DEC2BIN(-513)", E("#NUM!"), "ECMA-376 §18.17.7 DEC2BIN"),
            ("=DEC2BIN(1,0)", E("#NUM!"), "ECMA-376 §18.17.7 DEC2BIN"),
            ("=DEC2HEX(100,4)", T("0064"), "hand: 100 = 0x64"),
            ("=DEC2HEX(-54)", T("FFFFFFFFCA"), "hand: 2^40-54"),
            ("=DEC2HEX(28)", T("1C"), "hand: 28 = 0x1C"),
            ("=DEC2HEX(64,1)", E("#NUM!"), "ECMA-376 §18.17.7 DEC2HEX"),
            ("=DEC2OCT(58,3)", T("072"), "hand: 58 = 0o72"),
            ("=DEC2OCT(-100)", T("7777777634"), "hand: 8^10-100"),
            ("=HEX2BIN(\"F\",8)", T("00001111"), "hand: 15"),
            ("=HEX2BIN(\"B7\")", T("10110111"), "hand: 183"),
            ("=HEX2BIN(\"FFFFFFFE00\")", T("1000000000"), "hand: -512"),
            (
                "=HEX2BIN(\"200\")",
                E("#NUM!"),
                "hand: 512 is out of the 10-bit range",
            ),
            ("=HEX2DEC(\"A5\")", N(165.0), "hand: 10*16+5"),
            (
                "=HEX2DEC(\"a5\")",
                N(165.0),
                "hand: hex digits in either case",
            ),
            (
                "=HEX2DEC(\"FFFFFFFF5B\")",
                N(-165.0),
                "hand: 0xA5 negated in 40 bits",
            ),
            (
                "=HEX2DEC(\"3DA408B9\")",
                N(1_034_160_313.0),
                "hand: positional value",
            ),
            ("=HEX2OCT(\"F\",3)", T("017"), "hand: 15 = 0o17"),
            ("=HEX2OCT(\"3B4E\")", T("35516"), "hand: 15182 = 0o35516"),
            ("=HEX2OCT(\"FFFFFFFF00\")", T("7777777400"), "hand: -256"),
            ("=OCT2BIN(\"3\",3)", T("011"), "hand: 3"),
            ("=OCT2BIN(\"7777777000\")", T("1000000000"), "hand: -512"),
            ("=OCT2DEC(\"54\")", N(44.0), "hand: 5*8+4"),
            (
                "=OCT2DEC(\"7777777533\")",
                N(-165.0),
                "hand: 8^10-165 = 0o7777777533",
            ),
            ("=OCT2HEX(\"100\",4)", T("0040"), "hand: 64"),
            ("=OCT2HEX(\"7777777533\")", T("FFFFFFFF5B"), "hand: -165"),
            ("=BIN2DEC(TRUE)", E("#VALUE!"), "ECMA-376 §18.17.7 BIN2DEC"),
        ],
    );
}

#[test]
fn bits_and_comparisons() {
    check(
        DATA,
        &[
            ("=BITAND(1,5)", N(1.0), "hand: 001 & 101"),
            ("=BITAND(13,25)", N(9.0), "hand: 01101 & 11001 = 01001"),
            ("=BITOR(23,10)", N(31.0), "hand: 10111 | 01010"),
            ("=BITXOR(5,3)", N(6.0), "hand: 101 ^ 011"),
            ("=BITLSHIFT(4,2)", N(16.0), "hand: 4*2^2"),
            (
                "=BITLSHIFT(4,-2)",
                N(1.0),
                "hand: negative shift goes right",
            ),
            ("=BITRSHIFT(13,2)", N(3.0), "hand: 1101 >> 2"),
            ("=BITAND(-1,1)", E("#NUM!"), "MS-XLSX _xlfn.BITAND"),
            ("=BITAND(1.5,1)", E("#NUM!"), "MS-XLSX _xlfn.BITAND"),
            ("=BITAND(2^48,1)", E("#NUM!"), "MS-XLSX _xlfn.BITAND"),
            ("=BITLSHIFT(1,48)", E("#NUM!"), "MS-XLSX _xlfn.BITLSHIFT"),
            (
                "=BITOR(2^48-1,0)",
                N(281_474_976_710_655.0),
                "hand: largest 48-bit value",
            ),
            ("=DELTA(5,4)", N(0.0), "ECMA-376 §18.17.7 DELTA"),
            ("=DELTA(5,5)", N(1.0), "ECMA-376 §18.17.7 DELTA"),
            ("=DELTA(0.5)", N(0.0), "ECMA-376 §18.17.7 DELTA"),
            ("=DELTA(0)", N(1.0), "ECMA-376 §18.17.7 DELTA"),
            ("=DELTA(\"a\",1)", E("#VALUE!"), "ECMA-376 §18.17.7 DELTA"),
            ("=GESTEP(5,4)", N(1.0), "ECMA-376 §18.17.7 GESTEP"),
            ("=GESTEP(5,5)", N(1.0), "ECMA-376 §18.17.7 GESTEP"),
            ("=GESTEP(-4,-5)", N(1.0), "ECMA-376 §18.17.7 GESTEP"),
            ("=GESTEP(-1)", N(0.0), "ECMA-376 §18.17.7 GESTEP"),
        ],
    );
}

#[test]
fn error_function_and_bessel() {
    check(
        DATA,
        &[
            (
                "=ERF(1)",
                N(0.842_700_792_949_714_9),
                "hand: A&S Table 7.1 erf(1)",
            ),
            (
                "=ERF(0.745)",
                N(0.707_928_920_095_737_7),
                "hand: A&S Table 7.1 (Python math.erf)",
            ),
            ("=ERF(-1)", N(-0.842_700_792_949_714_9), "hand: erf is odd"),
            (
                "=ERF(0,1.5)",
                N(0.966_105_146_475_310_8),
                "hand: erf(1.5) - erf(0)",
            ),
            (
                "=ERF(1,0.5)",
                N(-0.322_200_915_136_668_4),
                "hand: erf(0.5) - erf(1)",
            ),
            (
                "=ERF(3)",
                N(0.999_977_909_503_001_4),
                "hand: A&S Table 7.1 erf(3)",
            ),
            (
                "=ERF.PRECISE(0.5)",
                N(0.520_499_877_813_046_5),
                "MS-XLSX _xlfn.ERF.PRECISE",
            ),
            ("=ERFC(1)", N(0.157_299_207_050_285_1), "hand: 1 - erf(1)"),
            (
                "=ERFC(3)",
                N(2.209_049_699_858_544e-5),
                "hand: A&S Table 7.1 erfc(3)",
            ),
            (
                "=ERFC.PRECISE(-1)",
                N(1.842_700_792_949_715),
                "MS-XLSX _xlfn.ERFC.PRECISE",
            ),
            ("=ERF(\"a\")", E("#VALUE!"), "ECMA-376 §18.17.7 ERF"),
            (
                "=BESSELJ(1,0)",
                N(0.765_197_686_557_966_6),
                "hand: A&S Table 9.1 J0(1)",
            ),
            (
                "=BESSELJ(1,1)",
                N(0.440_050_585_744_933_5),
                "hand: A&S Table 9.1 J1(1)",
            ),
            (
                "=BESSELJ(1.9,2)",
                N(0.329_925_727_692_387_2),
                "hand: A&S 9.1 J2(1.9) (SciPy jv)",
            ),
            (
                "=BESSELJ(1.9,2.7)",
                N(0.329_925_727_692_387_2),
                "hand: the order is truncated",
            ),
            (
                "=BESSELJ(10,3)",
                N(0.058_379_379_305_186_67),
                "hand: A&S Table 9.2 J3(10)",
            ),
            (
                "=BESSELJ(25,0)",
                N(0.096_266_783_275_958_11),
                "hand: SciPy jv(0,25)",
            ),
            (
                "=BESSELJ(-1,1)",
                N(-0.440_050_585_744_933_5),
                "hand: J1 is odd",
            ),
            ("=BESSELJ(1,-1)", E("#NUM!"), "ECMA-376 §18.17.7 BESSELJ"),
            (
                "=BESSELY(1,0)",
                N(0.088_256_964_215_677),
                "hand: A&S Table 9.1 Y0(1)",
            ),
            (
                "=BESSELY(1,1)",
                N(-0.781_212_821_300_288_9),
                "hand: A&S Table 9.1 Y1(1)",
            ),
            (
                "=BESSELY(2.5,1)",
                N(0.145_918_137_966_785_8),
                "hand: A&S 9.1 Y1(2.5) (SciPy yv)",
            ),
            (
                "=BESSELY(10,3)",
                N(-0.251_362_657_183_837_27),
                "hand: A&S Table 9.2 Y3(10)",
            ),
            ("=BESSELY(0,1)", E("#NUM!"), "ECMA-376 §18.17.7 BESSELY"),
            (
                "=BESSELI(1,0)",
                N(1.266_065_877_752_008_4),
                "hand: A&S Table 9.8 I0(1)",
            ),
            (
                "=BESSELI(1,1)",
                N(0.565_159_103_992_485),
                "hand: A&S Table 9.8 I1(1)",
            ),
            (
                "=BESSELI(1.5,1)",
                N(0.981_666_428_577_907_8),
                "hand: SciPy iv(1,1.5)",
            ),
            (
                "=BESSELI(5,2)",
                N(17.505_614_966_624_236),
                "hand: SciPy iv(2,5)",
            ),
            (
                "=BESSELK(1,0)",
                N(0.421_024_438_240_708_34),
                "hand: A&S Table 9.8 K0(1)",
            ),
            (
                "=BESSELK(1,1)",
                N(0.601_907_230_197_234_6),
                "hand: A&S Table 9.8 K1(1)",
            ),
            (
                "=BESSELK(1.5,1)",
                N(0.277_387_800_456_843_8),
                "hand: SciPy kv(1,1.5)",
            ),
            (
                "=BESSELK(0.5,3)",
                N(62.057_909_529_930_25),
                "hand: SciPy kv(3,0.5)",
            ),
            ("=BESSELK(-1,1)", E("#NUM!"), "ECMA-376 §18.17.7 BESSELK"),
        ],
    );
}

#[test]
fn complex_numbers() {
    check(
        DATA,
        &[
            ("=COMPLEX(3,4)", T("3+4i"), "ECMA-376 §18.17.7 COMPLEX"),
            (
                "=COMPLEX(3,4,\"j\")",
                T("3+4j"),
                "ECMA-376 §18.17.7 COMPLEX",
            ),
            ("=COMPLEX(0,1)", T("i"), "ECMA-376 §18.17.7 COMPLEX"),
            (
                "=COMPLEX(0,-1)",
                T("-i"),
                "hand: coefficient -1 is written as a sign",
            ),
            ("=COMPLEX(1,0)", T("1"), "ECMA-376 §18.17.7 COMPLEX"),
            ("=COMPLEX(0,0)", T("0"), "hand: zero"),
            ("=COMPLEX(1.5,-2.25)", T("1.5-2.25i"), "hand: signs"),
            (
                "=COMPLEX(1,2,\"k\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 COMPLEX",
            ),
            ("=IMABS(A1)", N(5.0), "hand: sqrt(9+16)"),
            ("=IMAGINARY(A1)", N(4.0), "hand: by definition"),
            ("=IMAGINARY(\"-j\")", N(-1.0), "hand: bare suffix"),
            ("=IMAGINARY(\"i\")", N(1.0), "hand: bare suffix"),
            ("=IMAGINARY(\"4\")", N(0.0), "hand: a real number"),
            ("=IMREAL(\"6-9i\")", N(6.0), "hand: by definition"),
            (
                "=IMREAL(\"1e2-3.5j\")",
                N(100.0),
                "hand: exponent sign is not the imaginary sign",
            ),
            ("=IMAGINARY(\"1e2-3.5j\")", N(-3.5), "hand: by definition"),
            ("=IMREAL(5)", N(5.0), "hand: a number argument"),
            ("=IMREAL(\"abc\")", E("#NUM!"), "ECMA-376 §18.17.7 IMREAL"),
            (
                "=IMARGUMENT(A1)",
                N(0.927_295_218_001_612_2),
                "hand: atan2(4,3)",
            ),
            (
                "=IMARGUMENT(\"0\")",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 IMARGUMENT",
            ),
            ("=IMCONJUGATE(A1)", T("3-4i"), "hand: by definition"),
            ("=IMSUM(A1,A2)", T("8+i"), "hand: (3+5)+(4-3)i"),
            (
                "=IMSUM(A1:A3,\"1\")",
                T("9+i"),
                "hand: blank skipped, 3+5+1",
            ),
            (
                "=IMSUB(\"13+4i\",\"5+3i\")",
                T("8+i"),
                "hand: by definition",
            ),
            ("=IMPRODUCT(A1,A2)", T("27+11i"), "hand: (15+12)+(-9+20)i"),
            (
                "=IMPRODUCT(\"1+2i\",30)",
                T("30+60i"),
                "hand: by definition",
            ),
            (
                "=IMDIV(\"-238+240i\",\"10+24i\")",
                T("5+12i"),
                "hand: (3380+8112i)/676",
            ),
            ("=IMDIV(A1,\"0\")", E("#NUM!"), "ECMA-376 §18.17.7 IMDIV"),
            (
                "=IMSUM(\"1+i\",\"1+j\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 IMSUM",
            ),
            ("=IMPOWER(\"2+3i\",3)", T("-46+9i"), "hand: 8+36i-54-27i"),
            ("=IMPOWER(\"i\",2)", T("-1"), "hand: i^2"),
            (
                "=IMPOWER(\"0\",-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 IMPOWER",
            ),
            (
                "=IMREAL(IMSQRT(\"1+i\"))",
                N(1.098_684_113_467_81),
                "hand: 2^(1/4) cos(pi/8)",
            ),
            (
                "=IMAGINARY(IMSQRT(\"1+i\"))",
                N(0.455_089_860_562_227),
                "hand: 2^(1/4) sin(pi/8)",
            ),
            ("=IMSQRT(\"-4\")", T("2i"), "hand: principal root"),
            (
                "=IMREAL(IMEXP(\"1+i\"))",
                N(1.468_693_939_915_89),
                "hand: e cos 1",
            ),
            (
                "=IMAGINARY(IMEXP(\"1+i\"))",
                N(2.287_355_287_178_84),
                "hand: e sin 1",
            ),
            (
                "=IMLN(A1)",
                T("1.6094379124341+0.927295218001612i"),
                "hand: ln 5 + i atan2(4,3)",
            ),
            ("=IMLN(\"0\")", E("#NUM!"), "ECMA-376 §18.17.7 IMLN"),
            (
                "=IMREAL(IMLOG10(A1))",
                N(0.698_970_004_336_019),
                "hand: log10 5",
            ),
            (
                "=IMAGINARY(IMLOG10(A1))",
                N(0.402_719_196_273_373),
                "hand: atan2(4,3)/ln 10",
            ),
            (
                "=IMREAL(IMLOG2(A1))",
                N(2.321_928_094_887_36),
                "hand: log2 5",
            ),
            (
                "=IMAGINARY(IMLOG2(A1))",
                N(1.337_804_212_450_98),
                "hand: atan2(4,3)/ln 2",
            ),
            (
                "=IMSIN(\"4+3i\")",
                T("-7.61923172032141-6.548120040911i"),
                "hand: sin4 cosh3 + i cos4 sinh3",
            ),
            (
                "=IMREAL(IMCOS(\"1+i\"))",
                N(0.833_730_025_131_149),
                "hand: cos1 cosh1",
            ),
            (
                "=IMAGINARY(IMCOS(\"1+i\"))",
                N(-0.988_897_705_762_865),
                "hand: -sin1 sinh1",
            ),
            (
                "=IMREAL(IMTAN(\"1+i\"))",
                N(0.271_752_585_319_512),
                "hand: sin z / cos z",
            ),
            (
                "=IMAGINARY(IMTAN(\"1+i\"))",
                N(1.083_923_327_338_69),
                "hand: sin z / cos z",
            ),
            (
                "=IMREAL(IMSEC(\"1+i\"))",
                N(0.498_337_030_555_187),
                "hand: 1 / cos z",
            ),
            (
                "=IMAGINARY(IMCSC(\"1+i\"))",
                N(-0.303_931_001_628_426),
                "hand: 1 / sin z",
            ),
            (
                "=IMAGINARY(IMCOT(\"1+i\"))",
                N(-0.868_014_142_895_925),
                "hand: cos z / sin z",
            ),
            (
                "=IMAGINARY(IMSINH(\"1+i\"))",
                N(1.298_457_581_415_98),
                "hand: cosh1 sin1",
            ),
            (
                "=IMREAL(IMCOSH(\"1+i\"))",
                N(0.833_730_025_131_149),
                "hand: cosh1 cos1",
            ),
            (
                "=IMREAL(IMCSCH(\"1+i\"))",
                N(0.303_931_001_628_426),
                "hand: 1 / sinh z",
            ),
            (
                "=IMAGINARY(IMSECH(\"1+i\"))",
                N(-0.591_083_841_721_045),
                "hand: 1 / cosh z",
            ),
        ],
    );
}

#[test]
fn convert_units() {
    check(
        DATA,
        &[
            (
                "=CONVERT(1,\"lbm\",\"kg\")",
                N(0.453_592_37),
                "hand: avoirdupois pound",
            ),
            ("=CONVERT(68,\"F\",\"C\")", N(20.0), "hand: (68-32)*5/9"),
            ("=CONVERT(32,\"F\",\"C\")", N(0.0), "hand: freezing point"),
            ("=CONVERT(100,\"C\",\"K\")", N(373.15), "hand: +273.15"),
            ("=CONVERT(0,\"K\",\"F\")", N(-459.67), "hand: absolute zero"),
            (
                "=CONVERT(1,\"Rank\",\"K\")",
                N(5.0 / 9.0),
                "hand: Rankine degree is 5/9 K",
            ),
            (
                "=CONVERT(2.5,\"ft\",\"sec\")",
                E("#N/A"),
                "ECMA-376 §18.17.7 CONVERT",
            ),
            (
                "=CONVERT(1,\"xyz\",\"m\")",
                E("#N/A"),
                "ECMA-376 §18.17.7 CONVERT",
            ),
            (
                "=CONVERT(\"a\",\"m\",\"ft\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 CONVERT",
            ),
            ("=CONVERT(100,\"ft\",\"m\")", N(30.48), "hand: 0.3048 m/ft"),
            (
                "=CONVERT(1,\"in\",\"cm\")",
                N(2.54),
                "hand: inch, centi prefix",
            ),
            (
                "=CONVERT(1,\"km\",\"mi\")",
                N(1000.0 / 1609.344),
                "hand: kilo prefix, statute mile",
            ),
            (
                "=CONVERT(1,\"gal\",\"l\")",
                N(3.785_411_784),
                "hand: 231 in^3",
            ),
            ("=CONVERT(1,\"day\",\"hr\")", N(24.0), "hand: by definition"),
            (
                "=CONVERT(1,\"atm\",\"Pa\")",
                N(101_325.0),
                "hand: standard atmosphere",
            ),
            (
                "=CONVERT(1,\"atm\",\"mmHg\")",
                N(759.999_891_725_611_2),
                "hand: 101325/(13.5951*9.80665)",
            ),
            (
                "=CONVERT(1,\"kbyte\",\"bit\")",
                N(8000.0),
                "hand: decimal prefix on byte",
            ),
            (
                "=CONVERT(1,\"kibyte\",\"byte\")",
                N(1024.0),
                "hand: binary prefix",
            ),
            (
                "=CONVERT(1,\"m2\",\"cm2\")",
                N(10_000.0),
                "hand: prefix squared",
            ),
            (
                "=CONVERT(1,\"km^2\",\"m2\")",
                N(1e6),
                "hand: prefix squared",
            ),
            ("=CONVERT(1,\"m3\",\"l\")", N(1000.0), "hand: by definition"),
            (
                "=CONVERT(1,\"HP\",\"W\")",
                N(745.699_871_582_270_2),
                "hand: 550 ft*lbf/s",
            ),
            (
                "=CONVERT(1,\"kn\",\"m/s\")",
                N(1852.0 / 3600.0),
                "hand: nautical mile per hour",
            ),
            (
                "=CONVERT(1,\"mph\",\"km/h\")",
                N(1.609_344),
                "hand: by definition",
            ),
            ("=CONVERT(1,\"g\",\"cg\")", N(100.0), "hand: centi prefix"),
            ("=CONVERT(1,\"cal\",\"J\")", N(4.1868), "hand: IT calorie"),
            (
                "=CONVERT(1,\"BTU\",\"J\")",
                N(1_055.055_852_62),
                "hand: IT British thermal unit",
            ),
            (
                "=CONVERT(1,\"uk_gal\",\"l\")",
                N(4.546_09),
                "hand: imperial gallon",
            ),
            ("=CONVERT(1,\"ha\",\"ar\")", N(100.0), "hand: by definition"),
            ("=CONVERT(1,\"ton\",\"lbm\")", N(2000.0), "hand: short ton"),
            ("=CONVERT(1,\"yr\",\"day\")", N(365.25), "hand: Julian year"),
            (
                "=CONVERT(1,\"ft\",\"kft\")",
                E("#N/A"),
                "hand: foot takes no prefix",
            ),
        ],
    );
}
