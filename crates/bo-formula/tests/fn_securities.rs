//! Securities and the remaining financial functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 for the definitions (coupon schedule, PRICE / YIELD,
//! DURATION, discounted and at-maturity securities, Treasury bills, ACCRINT, AMORLINC,
//! AMORDEGRC, VDB, MIRR). Expected values are worked by hand from those definitions: calendar
//! arithmetic for coupon dates and day counts, the closed forms evaluated step by step, and
//! identities (a bond on a regular schedule priced by the odd-period functions equals PRICE;
//! YIELD inverts PRICE; a par bond's Macaulay duration is (1+i)/i·(1−(1+i)^−N)).

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[];

#[test]
fn coupon_schedule() {
    check(
        DATA,
        &[
            (
                "=COUPPCD(DATE(2011,1,25),DATE(2011,11,15),2,1)",
                N(40_497.0),
                "hand: 2010-11-15",
            ),
            (
                "=COUPNCD(DATE(2011,1,25),DATE(2011,11,15),2,1)",
                N(40_678.0),
                "hand: 2011-05-15",
            ),
            (
                "=COUPNUM(DATE(2011,1,25),DATE(2011,11,15),2,1)",
                N(2.0),
                "hand: May and November",
            ),
            (
                "=COUPDAYBS(DATE(2011,1,25),DATE(2011,11,15),2,1)",
                N(71.0),
                "hand: 2010-11-15 to 2011-01-25",
            ),
            (
                "=COUPDAYS(DATE(2011,1,25),DATE(2011,11,15),2,1)",
                N(181.0),
                "hand: 2010-11-15 to 2011-05-15",
            ),
            (
                "=COUPDAYSNC(DATE(2011,1,25),DATE(2011,11,15),2,1)",
                N(110.0),
                "hand: 2011-01-25 to 2011-05-15",
            ),
            (
                "=COUPDAYBS(DATE(2011,1,25),DATE(2011,11,15),2,0)",
                N(70.0),
                "hand: 30/360 Nov 15 to Jan 25",
            ),
            (
                "=COUPDAYS(DATE(2011,1,25),DATE(2011,11,15),2,0)",
                N(180.0),
                "hand: 360/2",
            ),
            (
                "=COUPDAYSNC(DATE(2011,1,25),DATE(2011,11,15),2,0)",
                N(110.0),
                "hand: 180 - 70",
            ),
            (
                "=COUPDAYS(DATE(2011,1,25),DATE(2011,11,15),4,3)",
                N(91.25),
                "hand: 365/4",
            ),
            (
                "=COUPPCD(DATE(2011,3,15),DATE(2011,11,30),2,1)",
                N(40_512.0),
                "hand: month end kept, 2010-11-30",
            ),
            (
                "=COUPNCD(DATE(2011,3,15),DATE(2011,11,30),2,1)",
                N(40_694.0),
                "hand: 2011-05-31",
            ),
            (
                "=COUPNUM(DATE(2011,1,25),DATE(2011,11,15),3,1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 COUPNUM",
            ),
            (
                "=COUPNUM(DATE(2011,11,15),DATE(2011,1,25),2,1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 COUPNUM",
            ),
            (
                "=COUPNUM(DATE(2011,1,25),DATE(2011,11,15),2,5)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 COUPNUM",
            ),
        ],
    );
}

#[test]
fn bond_prices_and_yields() {
    check(
        DATA,
        &[
            (
                "=PRICE(DATE(2008,2,15),DATE(2017,11,15),0.0575,0.065,100,2,0)",
                N(94.634_361_621_322_14),
                "hand: 20 coupons, A = 90, DSC = 90, E = 180",
            ),
            (
                "=YIELD(DATE(2008,2,15),DATE(2017,11,15),0.0575,94.63436162132214,100,2,0)",
                Near(0.065, 1e-12),
                "hand: YIELD inverts PRICE",
            ),
            (
                "=PRICE(DATE(2011,1,25),DATE(2011,5,15),0.06,0.08,100,2,1)",
                N(99.378_759_975_445_06),
                "hand: one coupon left, (103)/(1+110/181*0.04) - 3*71/181",
            ),
            (
                "=YIELD(DATE(2011,1,25),DATE(2011,5,15),0.06,99.37875997544506,100,2,1)",
                Near(0.08, 1e-12),
                "hand: closed form for one coupon",
            ),
            ("=PRICE(DATE(2008,1,1),DATE(2018,1,1),0.08,0.08,100,2,1)", Near(100.0, 1e-9), "hand: a par bond on a coupon date"),
            ("=PRICE(DATE(2008,1,1),DATE(2018,1,1),0.08,-0.01,100,2,1)", E("#NUM!"), "ECMA-376 §18.17.7 PRICE"),
            ("=DURATION(DATE(2008,1,1),DATE(2018,1,1),0.08,0.08,2,1)", N(7.066_969_699_383_202), "hand: par bond (1+i)/(i f)(1-(1+i)^-N)"),
            ("=MDURATION(DATE(2008,1,1),DATE(2018,1,1),0.08,0.08,2,1)", N(6.795_163_172_483_848), "hand: DURATION/(1+y/f)"),
            ("=DURATION(DATE(2008,1,1),DATE(2018,1,1),0,0.08,2,1)", Near(10.0, 1e-12), "hand: zero coupon, the term"),
            (
                "=ODDLPRICE(DATE(2011,1,25),DATE(2011,5,15),DATE(2010,11,15),0.06,0.08,100,2,1)",
                N(99.378_759_975_445_06),
                "hand: a regular last period equals PRICE",
            ),
            (
                "=ODDLYIELD(DATE(2011,1,25),DATE(2011,5,15),DATE(2010,11,15),0.06,99.37875997544506,100,2,1)",
                Near(0.08, 1e-12),
                "hand: ODDLYIELD inverts ODDLPRICE",
            ),
            (
                "=ODDLYIELD(DATE(2011,1,25),DATE(2011,7,1),DATE(2010,12,20),0.06,ODDLPRICE(DATE(2011,1,25),DATE(2011,7,1),DATE(2010,12,20),0.06,0.05,100,2,0),100,2,0)",
                Near(0.05, 1e-12),
                "hand: round trip over an odd last period",
            ),
            (
                "=ODDFPRICE(DATE(2008,2,15),DATE(2017,11,15),DATE(2007,11,15),DATE(2008,5,15),0.0575,0.065,100,2,0)",
                N(94.634_361_621_322_14),
                "hand: a regular first period equals PRICE",
            ),
            (
                "=ODDFYIELD(DATE(2008,11,11),DATE(2021,3,1),DATE(2008,10,15),DATE(2009,3,1),0.0785,ODDFPRICE(DATE(2008,11,11),DATE(2021,3,1),DATE(2008,10,15),DATE(2009,3,1),0.0785,0.0625,100,2,1),100,2,1)",
                Near(0.0625, 1e-12),
                "hand: ODDFYIELD inverts ODDFPRICE (short first period)",
            ),
            (
                "=ODDFPRICE(DATE(2008,11,11),DATE(2021,3,1),DATE(2008,12,15),DATE(2009,3,1),0.0785,0.0625,100,2,1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 ODDFPRICE",
            ),
        ],
    );
}

#[test]
fn discounted_and_accrued() {
    check(
        DATA,
        &[
            ("=ACCRINT(DATE(2008,3,1),DATE(2008,8,31),DATE(2008,5,1),0.1,1000,2,0)", N(1000.0 * 0.1 * 60.0 / 360.0), "hand: 60 days of 30/360"),
            ("=ACCRINT(DATE(2008,3,1),DATE(2008,8,31),DATE(2008,5,1),0.1,1000,2,0,FALSE)", N(1000.0 * 0.1 * 60.0 / 360.0), "hand: settlement before the first coupon"),
            ("=ACCRINT(DATE(2008,3,1),DATE(2008,8,31),DATE(2008,10,1),0.1,1000,2,0,FALSE)", N(50.0 * 31.0 / 180.0), "hand: 30/360 Aug 31 to Oct 1 is 31 days of a 180-day period"),
            ("=ACCRINT(DATE(2008,3,1),DATE(2008,8,31),DATE(2008,10,1),0.1,1000,2,0,TRUE)", N(50.0 * 211.0 / 180.0), "hand: sum of A_i/NL_i: 180/180 (Mar 1 - Aug 31) + 31/180"),
            ("=ACCRINTM(DATE(2008,4,1),DATE(2008,6,15),0.1,1000,3)", N(1000.0 * 0.1 * 75.0 / 365.0), "hand: 75 actual days of 365"),
            ("=ACCRINTM(DATE(2008,6,15),DATE(2008,4,1),0.1,1000,3)", E("#NUM!"), "ECMA-376 §18.17.7 ACCRINTM"),
            ("=DISC(DATE(2018,1,25),DATE(2018,6,15),97.975,100,1)", N(0.052_420_212_765_957_444), "hand: (2.025/100)/(141/365)"),
            ("=INTRATE(DATE(2008,2,15),DATE(2008,5,15),1000000,1014420,2)", N(0.057_68), "hand: 0.01442/(90/360), 90 actual days"),
            ("=RECEIVED(DATE(2008,2,15),DATE(2008,5,15),1000000,0.0575,2)", N(1_014_584.654_407_102_1), "hand: 1e6/(1-0.0575*90/360)"),
            ("=PRICEDISC(DATE(2008,2,16),DATE(2008,3,1),0.0525,100,2)", N(100.0 - 5.25 * 14.0 / 360.0), "hand: 14 days"),
            ("=YIELDDISC(DATE(2008,2,16),DATE(2008,3,1),99.795,100,2)", N(0.052_822_571_986_858_77), "hand: (0.205/99.795)/(14/360)"),
            ("=PRICEMAT(DATE(2008,2,15),DATE(2008,4,13),DATE(2007,11,11),0.061,0.061,0)", N(99.984_498_875_556_94), "hand: DIM 152, A 94, DSM 58 (30/360)"),
            ("=YIELDMAT(DATE(2008,2,15),DATE(2008,4,13),DATE(2007,11,11),0.061,99.98449887555694,0)", Near(0.061, 1e-12), "hand: YIELDMAT inverts PRICEMAT"),
            ("=TBILLPRICE(DATE(2008,3,31),DATE(2008,6,1),0.09)", N(98.45), "hand: 100(1-0.09*62/360)"),
            ("=TBILLYIELD(DATE(2008,3,31),DATE(2008,6,1),98.45)", N(0.091_416_962_925_342_82), "hand: 1.55/98.45*360/62"),
            ("=TBILLEQ(DATE(2008,3,31),DATE(2008,6,1),0.0914)", N(0.094_151_493_565_943_02), "hand: 365d/(360-d*62)"),
            ("=TBILLPRICE(DATE(2008,3,31),DATE(2009,6,1),0.09)", E("#NUM!"), "ECMA-376 §18.17.7 TBILLPRICE"),
        ],
    );
}

#[test]
fn depreciation_and_helpers() {
    check(
        DATA,
        &[
            ("=DOLLARDE(1.02,16)", N(1.125), "hand: 1 + 2/16"),
            ("=DOLLARFR(1.125,16)", N(1.02), "hand: 1 + 0.125*16/100"),
            ("=DOLLARDE(1.1,32)", N(1.3125), "hand: 1 + 10/32"),
            (
                "=DOLLARDE(1.1,0)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 DOLLARDE",
            ),
            (
                "=DOLLARDE(1.1,-1)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 DOLLARDE",
            ),
            (
                "=FVSCHEDULE(1,{0.09,0.11,0.1})",
                N(1.330_89),
                "hand: 1.09*1.11*1.1",
            ),
            (
                "=ISPMT(0.1/12,1,36,8000000)",
                N(-64_814.814_814_814_82),
                "hand: pv*rate*(per/nper-1)",
            ),
            (
                "=MIRR({-120000,39000,30000,21000,37000,46000},0.1,0.12)",
                N(0.126_094_130_365_905_1),
                "hand: (FV of inflows / PV of outflows)^(1/5)-1",
            ),
            (
                "=MIRR({1,2,3},0.1,0.12)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 MIRR",
            ),
            (
                "=PDURATION(0.025,2000,2200)",
                N(3.859_866_162_622_662),
                "hand: ln 1.1 / ln 1.025",
            ),
            (
                "=PDURATION(0,2000,2200)",
                E("#NUM!"),
                "MS-XLSX _xlfn.PDURATION",
            ),
            (
                "=RRI(96,10000,11000)",
                N(0.000_993_307_376_291_330_3),
                "hand: 1.1^(1/96)-1",
            ),
            ("=VDB(2400,300,10,0,1)", N(480.0), "hand: 2400*2/10"),
            (
                "=VDB(2400,300,10,0,0.875,1.5)",
                N(315.0),
                "hand: 0.875*2400*0.15",
            ),
            (
                "=VDB(2400,300,10,6,10)",
                N(329.145_6),
                "hand: periods 7-10 declining balance, last capped at salvage",
            ),
            ("=VDB(2400,300,10,0,10)", N(2100.0), "hand: cost - salvage"),
            ("=VDB(2400,300,10,5,4)", E("#NUM!"), "ECMA-376 §18.17.7 VDB"),
            (
                "=AMORLINC(2400,DATE(2008,8,19),DATE(2008,12,31),300,1,0.15,1)",
                N(360.0),
                "hand: 2400*0.15",
            ),
            (
                "=AMORLINC(2400,DATE(2008,8,19),DATE(2008,12,31),300,0,0.15,1)",
                N(2400.0 * 0.15 * 134.0 / 366.0),
                "hand: 134/366 of a year",
            ),
            (
                "=AMORDEGRC(2400,DATE(2008,8,19),DATE(2008,12,31),300,1,0.15,1)",
                N(776.0),
                "hand: rate 0.375; first 330, then round(0.375*2070)",
            ),
            (
                "=AMORDEGRC(2400,DATE(2008,8,19),DATE(2008,12,31),300,0,0.15,1)",
                N(330.0),
                "hand: round(134/366*0.375*2400)",
            ),
            (
                "=AMORLINC(2400,DATE(2008,8,19),DATE(2008,12,31),300,1,0.15,2)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 AMORLINC",
            ),
        ],
    );
}
