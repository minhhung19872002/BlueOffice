//! Probability distributions, statistical tests and the remaining descriptive statistics
//! (with their compatibility names).
//!
//! Sources: ECMA-376 Part 1, §18.17.7 for each function's definition and argument domain.
//! Expected values are the exact values of the distributions (computed independently with
//! SciPy `scipy.stats` / `scipy.special`, which the source column names) or worked by hand.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    // Paired samples for T.TEST.
    ("A1", "3"),
    ("A2", "4"),
    ("A3", "5"),
    ("A4", "8"),
    ("A5", "9"),
    ("A6", "1"),
    ("A7", "2"),
    ("A8", "4"),
    ("A9", "5"),
    ("B1", "6"),
    ("B2", "19"),
    ("B3", "3"),
    ("B4", "2"),
    ("B5", "14"),
    ("B6", "4"),
    ("B7", "5"),
    ("B8", "17"),
    ("B9", "1"),
    // F.TEST samples.
    ("C1", "6"),
    ("C2", "7"),
    ("C3", "9"),
    ("C4", "15"),
    ("C5", "21"),
    ("D1", "20"),
    ("D2", "28"),
    ("D3", "31"),
    ("D4", "38"),
    ("D5", "40"),
    // CHISQ.TEST observed (E1:F3) and expected (G1:H3).
    ("E1", "58"),
    ("F1", "35"),
    ("E2", "11"),
    ("F2", "25"),
    ("E3", "10"),
    ("F3", "23"),
    ("G1", "45.35"),
    ("H1", "47.65"),
    ("G2", "17.56"),
    ("H2", "18.44"),
    ("G3", "16.09"),
    ("H3", "16.91"),
];

#[test]
fn continuous_distributions() {
    check(
        DATA,
        &[
            (
                "=BETA.DIST(0.4,8,10,TRUE)",
                N(0.359_492_342_933_094_73),
                "hand: SciPy beta.cdf",
            ),
            (
                "=BETA.DIST(0.4,8,10,FALSE)",
                N(3.211_117_065_142_271_4),
                "hand: SciPy beta.pdf",
            ),
            (
                "=BETA.DIST(2,8,10,TRUE,1,3)",
                N(0.685_470_581_054_687_5),
                "hand: SciPy beta.cdf((2-1)/2)",
            ),
            (
                "=BETADIST(2,8,10,1,3)",
                N(0.685_470_581_054_687_5),
                "hand: SciPy beta.cdf((2-1)/2)",
            ),
            (
                "=BETA.INV(0.6854705810546875,8,10,1,3)",
                Near(2.0, 1e-9),
                "hand: inverse of the row above",
            ),
            (
                "=BETAINV(0.6854705810546875,8,10,1,3)",
                Near(2.0, 1e-9),
                "hand: inverse of the row above",
            ),
            (
                "=BETA.DIST(4,8,10,TRUE,1,3)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 BETADIST",
            ),
            (
                "=BETA.DIST(0.5,0,1,TRUE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 BETADIST",
            ),
            (
                "=CHISQ.DIST(0.5,1,TRUE)",
                N(0.520_499_877_813_046_6),
                "hand: SciPy chi2.cdf",
            ),
            (
                "=CHISQ.DIST(2,3,FALSE)",
                N(0.207_553_748_710_297_4),
                "hand: SciPy chi2.pdf",
            ),
            (
                "=CHISQ.DIST.RT(18.307,10)",
                N(0.050_000_589_091_398_12),
                "hand: SciPy chi2.sf",
            ),
            (
                "=CHIDIST(18.307,10)",
                N(0.050_000_589_091_398_12),
                "hand: SciPy chi2.sf",
            ),
            (
                "=CHISQ.INV(0.93,1)",
                N(3.283_020_286_759_539),
                "hand: SciPy chi2.ppf",
            ),
            (
                "=CHISQ.INV.RT(0.050001,10)",
                N(18.306_973_456_961_06),
                "hand: SciPy chi2.isf",
            ),
            (
                "=CHIINV(0.050001,10)",
                N(18.306_973_456_961_06),
                "hand: SciPy chi2.isf",
            ),
            (
                "=CHISQ.DIST(-1,2,TRUE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 CHIDIST",
            ),
            (
                "=CHISQ.DIST(1,0,TRUE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 CHIDIST",
            ),
            (
                "=EXPON.DIST(0.2,10,TRUE)",
                N(0.864_664_716_763_387_3),
                "hand: 1-e^-2",
            ),
            (
                "=EXPON.DIST(0.2,10,FALSE)",
                N(1.353_352_832_366_127),
                "hand: 10 e^-2",
            ),
            (
                "=EXPONDIST(0.2,10,TRUE)",
                N(0.864_664_716_763_387_3),
                "hand: 1-e^-2",
            ),
            (
                "=EXPON.DIST(0.2,0,TRUE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 EXPONDIST",
            ),
            (
                "=F.DIST(15.2069,6,4,TRUE)",
                N(0.990_000_043_002_762_7),
                "hand: SciPy f.cdf",
            ),
            (
                "=F.DIST(15.2069,6,4,FALSE)",
                N(0.001_223_791_708_783_18),
                "hand: SciPy f.pdf",
            ),
            (
                "=F.DIST.RT(15.2069,6,4)",
                N(0.009_999_956_997_237_314),
                "hand: SciPy f.sf",
            ),
            (
                "=FDIST(15.2069,6,4)",
                N(0.009_999_956_997_237_314),
                "hand: SciPy f.sf",
            ),
            (
                "=F.INV(0.01,6,4)",
                N(0.109_309_914_124_578_55),
                "hand: SciPy f.ppf",
            ),
            (
                "=F.INV.RT(0.01,6,4)",
                N(15.206_864_861_157_523),
                "hand: SciPy f.isf",
            ),
            (
                "=FINV(0.01,6,4)",
                N(15.206_864_861_157_523),
                "hand: SciPy f.isf",
            ),
            ("=F.DIST(1,0,4,TRUE)", E("#NUM!"), "ECMA-376 §18.17.7 FDIST"),
            (
                "=GAMMA(2.5)",
                N(1.329_340_388_179_137),
                "hand: 3/4 sqrt(pi)",
            ),
            (
                "=GAMMA(-0.75)",
                N(-4.834_146_544_295_876),
                "hand: SciPy gamma",
            ),
            ("=GAMMA(5)", N(24.0), "hand: 4!"),
            ("=GAMMA(0)", E("#NUM!"), "MS-XLSX _xlfn.GAMMA"),
            ("=GAMMA(-1)", E("#NUM!"), "MS-XLSX _xlfn.GAMMA"),
            (
                "=GAMMA.DIST(10.00001131,9,2,TRUE)",
                N(0.068_094_003_869_787_2),
                "hand: SciPy gamma.cdf",
            ),
            (
                "=GAMMA.DIST(10.00001131,9,2,FALSE)",
                N(0.032_639_130_418_293_96),
                "hand: SciPy gamma.pdf",
            ),
            (
                "=GAMMADIST(10.00001131,9,2,TRUE)",
                N(0.068_094_003_869_787_2),
                "hand: SciPy gamma.cdf",
            ),
            (
                "=GAMMA.INV(0.068094,9,2)",
                N(10.000_011_191_437_181),
                "hand: SciPy gamma.ppf",
            ),
            (
                "=GAMMAINV(0.068094,9,2)",
                N(10.000_011_191_437_181),
                "hand: SciPy gamma.ppf",
            ),
            ("=GAMMALN(4)", N(1.791_759_469_228_055), "hand: ln 6"),
            (
                "=GAMMALN.PRECISE(4)",
                N(1.791_759_469_228_055),
                "hand: ln 6",
            ),
            ("=GAMMALN(0)", E("#NUM!"), "ECMA-376 §18.17.7 GAMMALN"),
            (
                "=GAUSS(2)",
                N(0.477_249_868_051_820_8),
                "hand: Phi(2) - 0.5",
            ),
            (
                "=PHI(0.75)",
                N(0.301_137_432_154_804_43),
                "hand: standard normal density",
            ),
            (
                "=LOGNORM.DIST(4,3.5,1.2,TRUE)",
                N(0.039_083_555_706_800_47),
                "hand: SciPy lognorm.cdf",
            ),
            (
                "=LOGNORM.DIST(4,3.5,1.2,FALSE)",
                N(0.017_617_596_681_819_23),
                "hand: SciPy lognorm.pdf",
            ),
            (
                "=LOGNORMDIST(4,3.5,1.2)",
                N(0.039_083_555_706_800_47),
                "hand: SciPy lognorm.cdf",
            ),
            (
                "=LOGNORM.INV(0.039084,3.5,1.2)",
                N(4.000_025_218_680_636),
                "hand: exp(3.5 + 1.2 z)",
            ),
            (
                "=LOGINV(0.039084,3.5,1.2)",
                N(4.000_025_218_680_636),
                "hand: exp(3.5 + 1.2 z)",
            ),
            (
                "=LOGNORM.DIST(0,3.5,1.2,TRUE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 LOGNORMDIST",
            ),
            (
                "=T.DIST(60,1,TRUE)",
                N(0.994_695_326_367_376_7),
                "hand: SciPy t.cdf",
            ),
            (
                "=T.DIST(8,3,FALSE)",
                N(0.000_736_906_520_946_926_4),
                "hand: SciPy t.pdf",
            ),
            (
                "=T.DIST.2T(1.959999998,60)",
                N(0.054_644_929_975_920_89),
                "hand: 2 SciPy t.sf",
            ),
            (
                "=T.DIST.RT(1.959999998,60)",
                N(0.027_322_464_987_960_444),
                "hand: SciPy t.sf",
            ),
            (
                "=TDIST(1.959999998,60,2)",
                N(0.054_644_929_975_920_89),
                "hand: 2 SciPy t.sf",
            ),
            (
                "=TDIST(1.959999998,60,1)",
                N(0.027_322_464_987_960_444),
                "hand: SciPy t.sf",
            ),
            ("=TDIST(-1,60,1)", E("#NUM!"), "ECMA-376 §18.17.7 TDIST"),
            (
                "=T.INV(0.75,2)",
                N(0.816_496_580_927_726_1),
                "hand: SciPy t.ppf",
            ),
            (
                "=T.INV.2T(0.546449,60)",
                N(0.606_533_075_825_754_9),
                "hand: SciPy t.ppf(1-p/2)",
            ),
            (
                "=TINV(0.546449,60)",
                N(0.606_533_075_825_754_9),
                "hand: SciPy t.ppf(1-p/2)",
            ),
            ("=T.INV(0,2)", E("#NUM!"), "MS-XLSX _xlfn.T.INV"),
            (
                "=WEIBULL.DIST(105,20,100,TRUE)",
                N(0.929_581_390_069_276_9),
                "hand: 1-exp(-(1.05)^20)",
            ),
            (
                "=WEIBULL.DIST(105,20,100,FALSE)",
                N(0.035_588_864_024_504_334),
                "hand: SciPy weibull_min.pdf",
            ),
            (
                "=WEIBULL(105,20,100,TRUE)",
                N(0.929_581_390_069_276_9),
                "hand: 1-exp(-(1.05)^20)",
            ),
            (
                "=FISHER(0.75)",
                N(0.972_955_074_527_656_6),
                "hand: atanh 0.75",
            ),
            (
                "=FISHERINV(0.972955)",
                N(0.749_999_967_394_148_4),
                "hand: tanh",
            ),
            ("=FISHER(1)", E("#NUM!"), "ECMA-376 §18.17.7 FISHER"),
            (
                "=CONFIDENCE.NORM(0.05,2.5,50)",
                N(0.692_951_912_174_838_9),
                "hand: z(0.975) 2.5/sqrt 50",
            ),
            (
                "=CONFIDENCE(0.05,2.5,50)",
                N(0.692_951_912_174_838_9),
                "hand: z(0.975) 2.5/sqrt 50",
            ),
            (
                "=CONFIDENCE.T(0.05,1,50)",
                N(0.284_196_855_495_729_87),
                "hand: t(0.975,49)/sqrt 50",
            ),
            (
                "=CONFIDENCE.T(0.05,1,1)",
                E("#DIV/0!"),
                "MS-XLSX _xlfn.CONFIDENCE.T",
            ),
        ],
    );
}

#[test]
fn discrete_distributions() {
    check(
        DATA,
        &[
            (
                "=BINOM.DIST(6,10,0.5,FALSE)",
                N(0.205_078_125),
                "hand: C(10,6)/1024",
            ),
            ("=BINOM.DIST(6,10,0.5,TRUE)", N(0.828_125), "hand: 848/1024"),
            ("=BINOMDIST(6,10,0.5,TRUE)", N(0.828_125), "hand: 848/1024"),
            (
                "=BINOM.DIST(11,10,0.5,TRUE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 BINOMDIST",
            ),
            (
                "=BINOM.DIST.RANGE(60,0.75,48)",
                N(0.083_974_967_429_047_53),
                "hand: SciPy binom.pmf",
            ),
            (
                "=BINOM.DIST.RANGE(60,0.75,45,50)",
                N(0.523_629_793_471_887_2),
                "hand: SciPy sum of binom.pmf",
            ),
            ("=BINOM.INV(6,0.5,0.75)", N(4.0), "hand: SciPy binom.ppf"),
            ("=CRITBINOM(6,0.5,0.75)", N(4.0), "hand: SciPy binom.ppf"),
            (
                "=HYPGEOM.DIST(1,4,8,20,TRUE)",
                N(0.465_428_276_573_787_4),
                "hand: SciPy hypergeom.cdf",
            ),
            (
                "=HYPGEOM.DIST(1,4,8,20,FALSE)",
                N(0.363_261_093_911_248_67),
                "hand: C(8,1)C(12,3)/C(20,4)",
            ),
            (
                "=HYPGEOMDIST(1,4,8,20)",
                N(0.363_261_093_911_248_67),
                "hand: C(8,1)C(12,3)/C(20,4)",
            ),
            (
                "=HYPGEOM.DIST(5,4,8,20,FALSE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 HYPGEOMDIST",
            ),
            (
                "=NEGBINOM.DIST(10,5,0.25,TRUE)",
                N(0.313_514_058_478_176_6),
                "hand: SciPy nbinom.cdf",
            ),
            (
                "=NEGBINOM.DIST(10,5,0.25,FALSE)",
                N(0.055_048_660_375_177_87),
                "hand: C(14,10) 0.25^5 0.75^10",
            ),
            (
                "=NEGBINOMDIST(10,5,0.25)",
                N(0.055_048_660_375_177_87),
                "hand: C(14,10) 0.25^5 0.75^10",
            ),
            (
                "=POISSON.DIST(2,5,TRUE)",
                N(0.124_652_019_483_081_08),
                "hand: e^-5 (1+5+12.5)",
            ),
            (
                "=POISSON.DIST(2,5,FALSE)",
                N(0.084_224_337_488_568_32),
                "hand: e^-5 12.5",
            ),
            (
                "=POISSON(2,5,FALSE)",
                N(0.084_224_337_488_568_32),
                "hand: e^-5 12.5",
            ),
            (
                "=POISSON.DIST(-1,5,FALSE)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 POISSON",
            ),
            (
                "=PROB({0,1,2,3},{0.2,0.3,0.1,0.4},2)",
                N(0.1),
                "hand: P(x=2)",
            ),
            (
                "=PROB({0,1,2,3},{0.2,0.3,0.1,0.4},1,3)",
                N(0.8),
                "hand: 0.3+0.1+0.4",
            ),
            (
                "=PROB({0,1},{0.2,0.3},1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 PROB",
            ),
        ],
    );
}

#[test]
fn statistical_tests() {
    check(
        DATA,
        &[
            (
                "=T.TEST(A1:A9,B1:B9,2,1)",
                N(0.196_015_784_925_282_04),
                "hand: SciPy ttest_rel",
            ),
            (
                "=T.TEST(A1:A9,B1:B9,2,2)",
                N(0.191_995_886_760_396_22),
                "hand: SciPy ttest_ind",
            ),
            (
                "=T.TEST(A1:A9,B1:B9,2,3)",
                N(0.202_293_923_368_677_96),
                "hand: SciPy ttest_ind (Welch)",
            ),
            (
                "=TTEST(A1:A9,B1:B9,1,2)",
                N(0.095_997_943_380_198_11),
                "hand: half the two-tailed value",
            ),
            (
                "=T.TEST(A1:A9,B1:B9,3,1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 TTEST",
            ),
            (
                "=F.TEST(C1:C5,D1:D5)",
                N(0.648_317_846_786_174_5),
                "hand: SciPy f.cdf, two-tailed",
            ),
            (
                "=FTEST(C1:C5,D1:D5)",
                N(0.648_317_846_786_174_5),
                "hand: SciPy f.cdf, two-tailed",
            ),
            (
                "=CHISQ.TEST(E1:F3,G1:H3)",
                N(0.000_308_192_017_008_309_5),
                "hand: SciPy chi2.sf, 2 df",
            ),
            (
                "=CHITEST(E1:F3,G1:H3)",
                N(0.000_308_192_017_008_309_5),
                "hand: SciPy chi2.sf, 2 df",
            ),
            (
                "=CHISQ.TEST(E1:F3,G1:G3)",
                E("#N/A"),
                "ECMA-376 §18.17.7 CHITEST",
            ),
            (
                "=Z.TEST({3,6,7,8,6,5,4,2,1,9},4)",
                N(0.090_574_196_851_363_81),
                "hand: 1-Phi((5.1-4)/(s/sqrt 10))",
            ),
            (
                "=ZTEST({3,6,7,8,6,5,4,2,1,9},6)",
                N(0.863_043_389_129_529_9),
                "hand: 1-Phi((5.1-6)/(s/sqrt 10))",
            ),
        ],
    );
}

#[test]
fn descriptive_statistics() {
    check(
        DATA,
        &[
            (
                "=KURT({3,4,5,2,3,4,5,6,4,7})",
                N(-0.151_799_637_208_414_05),
                "hand: SciPy kurtosis(bias=False)",
            ),
            ("=KURT(1,2,3)", E("#DIV/0!"), "ECMA-376 §18.17.7 KURT"),
            (
                "=SKEW({3,4,5,2,3,4,5,6,4,7})",
                N(0.359_543_071_406_797_53),
                "hand: SciPy skew(bias=False)",
            ),
            (
                "=SKEW.P({3,4,5,2,3,4,5,6,4,7})",
                N(0.303_193_339_354_144_05),
                "hand: SciPy skew(bias=True)",
            ),
            ("=SKEW(1,1,1)", E("#DIV/0!"), "ECMA-376 §18.17.7 SKEW"),
            (
                "=STDEVPA({1345,1301,1368,1322,1310,1370,1318,1350,1303,1299})",
                N(26.054_558_142_482_477),
                "hand: population standard deviation",
            ),
            (
                "=VARPA({1345,1301,1368,1322,1310,1370,1318,1350,1303,1299})",
                N(678.84),
                "hand: population variance",
            ),
            ("=VARPA(1,TRUE)", N(0.0), "hand: TRUE counts as 1"),
            (
                "=TRIMMEAN({4,5,6,7,2,3,4,5,1,2,3},0.2)",
                N(34.0 / 9.0),
                "hand: drop one value at each end",
            ),
            (
                "=TRIMMEAN({1,2,3},1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 TRIMMEAN",
            ),
            (
                "=PERCENTRANK.INC({1,2,3,4,5,6,7,8,9,10},4)",
                N(0.333),
                "hand: 3/9 truncated",
            ),
            (
                "=PERCENTRANK.INC({1,2,3,4,5,6,7,8,9,10},4,5)",
                N(0.333_33),
                "hand: 3/9 truncated",
            ),
            (
                "=PERCENTRANK.INC({1,2,3,4,5,6,7,8,9,10},4.5)",
                N(0.388),
                "hand: (3+0.5)/9 truncated",
            ),
            (
                "=PERCENTRANK({1,2,3,4,5,6,7,8,9,10},10)",
                N(1.0),
                "hand: 9/9",
            ),
            (
                "=PERCENTRANK.INC({1,2,3,4,5,6,7,8,9,10},11)",
                E("#N/A"),
                "ECMA-376 §18.17.7 PERCENTRANK",
            ),
            (
                "=PERCENTRANK.EXC({1,2,3,4,5,6,7,8,9,10},4)",
                N(0.363),
                "hand: 4/11 truncated",
            ),
            (
                "=MODE.MULT({1,2,3,4,3,2,1,2,3})",
                A("{2;3}"),
                "hand: 2 and 3 occur three times",
            ),
            ("=MODE.MULT({1,2,3})", E("#N/A"), "MS-XLSX _xlfn.MODE.MULT"),
            (
                "=FREQUENCY({79,85,78,85,50,81,95,88,97},{70,79,89})",
                A("{1;2;4;2}"),
                "hand: counts per bin",
            ),
            (
                "=FREQUENCY({1,2,3},{2,1})",
                A("{1;1;1}"),
                "hand: bins in any order",
            ),
            (
                "=STEYX({2,3,9,1,8,7,5},{6,5,11,7,5,4,4})",
                N(3.305_718_950_210_041),
                "hand: SciPy linregress residuals",
            ),
            (
                "=STEYX({1,2},{3,4})",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 STEYX",
            ),
        ],
    );
}
