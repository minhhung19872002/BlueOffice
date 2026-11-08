"""Generates docs/parity/functions.md: the spreadsheet function catalogue (public function references of
the common commercial spreadsheet, grouped as documented) against what bo-formula implements.

Usage (repository root): python scripts/parity-functions.py
"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

CATALOG = {
    "Compatibility": """BETADIST BETAINV BINOMDIST CEILING CHIDIST CHIINV CHITEST CONCATENATE CONFIDENCE COVAR CRITBINOM
        EXPONDIST FDIST FINV FLOOR FORECAST FTEST GAMMADIST GAMMAINV HYPGEOMDIST LOGINV LOGNORMDIST MODE NEGBINOMDIST
        NORMDIST NORMINV NORMSDIST NORMSINV PERCENTILE PERCENTRANK POISSON QUARTILE RANK STDEV STDEVP TDIST TINV TTEST
        VAR VARP WEIBULL ZTEST""",
    "Cube": """CUBEKPIMEMBER CUBEMEMBER CUBEMEMBERPROPERTY CUBERANKEDMEMBER CUBESET CUBESETCOUNT CUBEVALUE""",
    "Database": """DAVERAGE DCOUNT DCOUNTA DGET DMAX DMIN DPRODUCT DSTDEV DSTDEVP DSUM DVAR DVARP""",
    "Date & Time": """DATE DATEDIF DATEVALUE DAY DAYS DAYS360 EDATE EOMONTH HOUR ISOWEEKNUM MINUTE MONTH NETWORKDAYS
        NETWORKDAYS.INTL NOW SECOND TIME TIMEVALUE TODAY WEEKDAY WEEKNUM WORKDAY WORKDAY.INTL YEAR YEARFRAC""",
    "Engineering": """BESSELI BESSELJ BESSELK BESSELY BIN2DEC BIN2HEX BIN2OCT BITAND BITLSHIFT BITOR BITRSHIFT BITXOR
        COMPLEX CONVERT DEC2BIN DEC2HEX DEC2OCT DELTA ERF ERF.PRECISE ERFC ERFC.PRECISE GESTEP HEX2BIN HEX2DEC HEX2OCT
        IMABS IMAGINARY IMARGUMENT IMCONJUGATE IMCOS IMCOSH IMCOT IMCSC IMCSCH IMDIV IMEXP IMLN IMLOG10 IMLOG2 IMPOWER
        IMPRODUCT IMREAL IMSEC IMSECH IMSIN IMSINH IMSQRT IMSUB IMSUM IMTAN OCT2BIN OCT2DEC OCT2HEX""",
    "Financial": """ACCRINT ACCRINTM AMORDEGRC AMORLINC COUPDAYBS COUPDAYS COUPDAYSNC COUPNCD COUPNUM COUPPCD CUMIPMT
        CUMPRINC DB DDB DISC DOLLARDE DOLLARFR DURATION EFFECT FV FVSCHEDULE INTRATE IPMT IRR ISPMT MDURATION MIRR NOMINAL
        NPER NPV ODDFPRICE ODDFYIELD ODDLPRICE ODDLYIELD PDURATION PMT PPMT PRICE PRICEDISC PRICEMAT PV RATE RECEIVED
        RRI SLN SYD TBILLEQ TBILLPRICE TBILLYIELD VDB XIRR XNPV YIELD YIELDDISC YIELDMAT""",
    "Information": """CELL ERROR.TYPE INFO ISBLANK ISERR ISERROR ISEVEN ISFORMULA ISLOGICAL ISNA ISNONTEXT ISNUMBER ISODD
        ISOMITTED ISREF ISTEXT N NA SHEET SHEETS TYPE""",
    "Logical": """AND BYCOL BYROW FALSE IF IFERROR IFNA IFS LAMBDA LET MAKEARRAY MAP NOT OR REDUCE SCAN SWITCH TRUE XOR""",
    "Lookup & Reference": """ADDRESS AREAS CHOOSE CHOOSECOLS CHOOSEROWS COLUMN COLUMNS DROP EXPAND FILTER FORMULATEXT
        GETPIVOTDATA GROUPBY HLOOKUP HSTACK HYPERLINK IMAGE INDEX INDIRECT LOOKUP MATCH OFFSET PIVOTBY ROW ROWS RTD SORT
        SORTBY TAKE TOCOL TOROW TRANSPOSE TRIMRANGE UNIQUE VLOOKUP VSTACK WRAPCOLS WRAPROWS XLOOKUP XMATCH""",
    "Math & Trig": """ABS ACOS ACOSH ACOT ACOTH AGGREGATE ARABIC ASIN ASINH ATAN ATAN2 ATANH BASE CEILING.MATH
        CEILING.PRECISE COMBIN COMBINA COS COSH COT COTH CSC CSCH DECIMAL DEGREES EVEN EXP FACT FACTDOUBLE FLOOR.MATH
        FLOOR.PRECISE GCD INT ISO.CEILING LCM LN LOG LOG10 MDETERM MINVERSE MMULT MOD MROUND MULTINOMIAL MUNIT ODD
        PERCENTOF PI POWER PRODUCT QUOTIENT RADIANS RAND RANDARRAY RANDBETWEEN ROMAN ROUND ROUNDDOWN ROUNDUP SEC SECH
        SEQUENCE SERIESSUM SIGN SIN SINH SQRT SQRTPI SUBTOTAL SUM SUMIF SUMIFS SUMPRODUCT SUMSQ SUMX2MY2 SUMX2PY2
        SUMXMY2 TAN TANH TRUNC""",
    "Statistical": """AVEDEV AVERAGE AVERAGEA AVERAGEIF AVERAGEIFS BETA.DIST BETA.INV BINOM.DIST BINOM.DIST.RANGE
        BINOM.INV CHISQ.DIST CHISQ.DIST.RT CHISQ.INV CHISQ.INV.RT CHISQ.TEST CONFIDENCE.NORM CONFIDENCE.T CORREL COUNT
        COUNTA COUNTBLANK COUNTIF COUNTIFS COVARIANCE.P COVARIANCE.S DEVSQ EXPON.DIST F.DIST F.DIST.RT F.INV F.INV.RT
        F.TEST FISHER FISHERINV FORECAST.ETS FORECAST.ETS.CONFINT FORECAST.ETS.SEASONALITY FORECAST.ETS.STAT
        FORECAST.LINEAR FREQUENCY GAMMA GAMMA.DIST GAMMA.INV GAMMALN GAMMALN.PRECISE GAUSS GEOMEAN GROWTH HARMEAN
        HYPGEOM.DIST INTERCEPT KURT LARGE LINEST LOGEST LOGNORM.DIST LOGNORM.INV MAX MAXA MAXIFS MEDIAN MIN MINA MINIFS
        MODE.MULT MODE.SNGL NEGBINOM.DIST NORM.DIST NORM.INV NORM.S.DIST NORM.S.INV PEARSON PERCENTILE.EXC
        PERCENTILE.INC PERCENTRANK.EXC PERCENTRANK.INC PERMUT PERMUTATIONA PHI POISSON.DIST PROB QUARTILE.EXC
        QUARTILE.INC RANK.AVG RANK.EQ RSQ SKEW SKEW.P SLOPE SMALL STANDARDIZE STDEV.P STDEV.S STDEVA STDEVPA STEYX
        T.DIST T.DIST.2T T.DIST.RT T.INV T.INV.2T T.TEST TREND TRIMMEAN VAR.P VAR.S VARA VARPA WEIBULL.DIST Z.TEST""",
    "Text": """ARRAYTOTEXT ASC BAHTTEXT CHAR CLEAN CODE CONCAT DBCS DOLLAR EXACT FIND FINDB FIXED LEFT LEFTB LEN LENB
        LOWER MID MIDB NUMBERVALUE PHONETIC PROPER REGEXEXTRACT REGEXREPLACE REGEXTEST REPLACE REPLACEB REPT RIGHT RIGHTB
        SEARCH SEARCHB SUBSTITUTE T TEXT TEXTAFTER TEXTBEFORE TEXTJOIN TEXTSPLIT TRIM UNICHAR UNICODE UPPER VALUE
        VALUETOTEXT""",
    "Web": """ENCODEURL FILTERXML WEBSERVICE""",
    "BlueOffice": """BO.DOCSO""",
}

# Implemented elsewhere than FUNCTIONS tables (parser-level or engine-level).
SPECIAL = {"LAMBDA", "LET", "TRUE", "FALSE"}


def implemented():
    names = set()
    d = os.path.join(ROOT, "crates", "bo-formula", "src", "functions")
    for f in os.listdir(d):
        if f.endswith(".rs"):
            text = open(os.path.join(d, f), encoding="utf-8").read()
            names.update(re.findall(r'^\s+[a-z_]+\("([A-Z][A-Z0-9._]*)"', text, re.M))
    src = os.path.join(ROOT, "crates", "bo-formula", "src")
    for f in os.listdir(src):
        if f.endswith(".rs"):
            text = open(os.path.join(src, f), encoding="utf-8").read()
            for n in SPECIAL:
                if f'"{n}"' in text:
                    names.add(n)
    return names


def main():
    have = implemented()
    rows = []
    total = done = 0
    by_group = []
    for group, names in CATALOG.items():
        ns = names.split()
        g_done = sum(1 for n in ns if n in have)
        by_group.append((group, g_done, len(ns)))
        for n in ns:
            ok = n in have
            total += 1
            done += ok
            rows.append(f"| {n} | {group} | {'[x]' if ok else '[ ]'} | {'bo-formula' if ok else ''} |")
    extra = sorted(have - {n for v in CATALOG.values() for n in v.split()})
    out = [
        "# Danh mục hàm bảng tính (BlueSheet)",
        "",
        "Sinh tự động bởi `python scripts/parity-functions.py` từ danh mục hàm công khai của bảng tính thương mại phổ biến",
        "(nhóm theo tài liệu hàm công khai) đối chiếu với các bảng `FUNCTIONS` trong `crates/bo-formula/src/functions/`.",
        "Test: bảng giá trị trong `crates/bo-formula/tests/` (giá trị kỳ vọng ghi nguồn).",
        "",
        f"**Tổng: {done}/{total} hàm ({100 * done / total:.0f} %).**",
        "",
        "| Nhóm | Đã có | Tổng |",
        "|---|---|---|",
    ]
    out += [f"| {g} | {d} | {t} |" for g, d, t in by_group]
    out += ["", "| Hàm | Nhóm | Trạng thái | Cài đặt |", "|---|---|---|---|"] + rows
    if extra:
        out += ["", "Hàm có trong bo-formula ngoài danh mục: " + ", ".join(extra)]
    os.makedirs(os.path.join(ROOT, "docs", "parity"), exist_ok=True)
    with open(os.path.join(ROOT, "docs", "parity", "functions.md"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")
    print(f"{done}/{total}")
    for g, d, t in by_group:
        print(f"{g}: {d}/{t}")
    if extra:
        print("extra:", extra)


if __name__ == "__main__":
    main()
