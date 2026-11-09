# Danh mục hàm bảng tính (BlueSheet)

Sinh tự động bởi `python scripts/parity-functions.py` từ danh mục hàm công khai của bảng tính thương mại phổ biến
(nhóm theo tài liệu hàm công khai) đối chiếu với các bảng `FUNCTIONS` trong `crates/bo-formula/src/functions/`.
Test: bảng giá trị trong `crates/bo-formula/tests/` (giá trị kỳ vọng ghi nguồn).

**Tổng: 329/515 hàm (64 %).**

| Nhóm | Đã có | Tổng |
|---|---|---|
| Compatibility | 17 | 42 |
| Cube | 0 | 7 |
| Database | 0 | 12 |
| Date & Time | 25 | 25 |
| Engineering | 54 | 54 |
| Financial | 19 | 55 |
| Information | 19 | 21 |
| Logical | 19 | 19 |
| Lookup & Reference | 27 | 40 |
| Math & Trig | 70 | 80 |
| Statistical | 50 | 110 |
| Text | 29 | 46 |
| Web | 0 | 3 |
| BlueOffice | 0 | 1 |

| Hàm | Nhóm | Trạng thái | Cài đặt |
|---|---|---|---|
| BETADIST | Compatibility | [ ] |  |
| BETAINV | Compatibility | [ ] |  |
| BINOMDIST | Compatibility | [ ] |  |
| CEILING | Compatibility | [x] | bo-formula |
| CHIDIST | Compatibility | [ ] |  |
| CHIINV | Compatibility | [ ] |  |
| CHITEST | Compatibility | [ ] |  |
| CONCATENATE | Compatibility | [x] | bo-formula |
| CONFIDENCE | Compatibility | [ ] |  |
| COVAR | Compatibility | [x] | bo-formula |
| CRITBINOM | Compatibility | [ ] |  |
| EXPONDIST | Compatibility | [ ] |  |
| FDIST | Compatibility | [ ] |  |
| FINV | Compatibility | [ ] |  |
| FLOOR | Compatibility | [x] | bo-formula |
| FORECAST | Compatibility | [x] | bo-formula |
| FTEST | Compatibility | [ ] |  |
| GAMMADIST | Compatibility | [ ] |  |
| GAMMAINV | Compatibility | [ ] |  |
| HYPGEOMDIST | Compatibility | [ ] |  |
| LOGINV | Compatibility | [ ] |  |
| LOGNORMDIST | Compatibility | [ ] |  |
| MODE | Compatibility | [x] | bo-formula |
| NEGBINOMDIST | Compatibility | [ ] |  |
| NORMDIST | Compatibility | [x] | bo-formula |
| NORMINV | Compatibility | [x] | bo-formula |
| NORMSDIST | Compatibility | [x] | bo-formula |
| NORMSINV | Compatibility | [x] | bo-formula |
| PERCENTILE | Compatibility | [x] | bo-formula |
| PERCENTRANK | Compatibility | [ ] |  |
| POISSON | Compatibility | [ ] |  |
| QUARTILE | Compatibility | [x] | bo-formula |
| RANK | Compatibility | [x] | bo-formula |
| STDEV | Compatibility | [x] | bo-formula |
| STDEVP | Compatibility | [x] | bo-formula |
| TDIST | Compatibility | [ ] |  |
| TINV | Compatibility | [ ] |  |
| TTEST | Compatibility | [ ] |  |
| VAR | Compatibility | [x] | bo-formula |
| VARP | Compatibility | [x] | bo-formula |
| WEIBULL | Compatibility | [ ] |  |
| ZTEST | Compatibility | [ ] |  |
| CUBEKPIMEMBER | Cube | [ ] |  |
| CUBEMEMBER | Cube | [ ] |  |
| CUBEMEMBERPROPERTY | Cube | [ ] |  |
| CUBERANKEDMEMBER | Cube | [ ] |  |
| CUBESET | Cube | [ ] |  |
| CUBESETCOUNT | Cube | [ ] |  |
| CUBEVALUE | Cube | [ ] |  |
| DAVERAGE | Database | [ ] |  |
| DCOUNT | Database | [ ] |  |
| DCOUNTA | Database | [ ] |  |
| DGET | Database | [ ] |  |
| DMAX | Database | [ ] |  |
| DMIN | Database | [ ] |  |
| DPRODUCT | Database | [ ] |  |
| DSTDEV | Database | [ ] |  |
| DSTDEVP | Database | [ ] |  |
| DSUM | Database | [ ] |  |
| DVAR | Database | [ ] |  |
| DVARP | Database | [ ] |  |
| DATE | Date & Time | [x] | bo-formula |
| DATEDIF | Date & Time | [x] | bo-formula |
| DATEVALUE | Date & Time | [x] | bo-formula |
| DAY | Date & Time | [x] | bo-formula |
| DAYS | Date & Time | [x] | bo-formula |
| DAYS360 | Date & Time | [x] | bo-formula |
| EDATE | Date & Time | [x] | bo-formula |
| EOMONTH | Date & Time | [x] | bo-formula |
| HOUR | Date & Time | [x] | bo-formula |
| ISOWEEKNUM | Date & Time | [x] | bo-formula |
| MINUTE | Date & Time | [x] | bo-formula |
| MONTH | Date & Time | [x] | bo-formula |
| NETWORKDAYS | Date & Time | [x] | bo-formula |
| NETWORKDAYS.INTL | Date & Time | [x] | bo-formula |
| NOW | Date & Time | [x] | bo-formula |
| SECOND | Date & Time | [x] | bo-formula |
| TIME | Date & Time | [x] | bo-formula |
| TIMEVALUE | Date & Time | [x] | bo-formula |
| TODAY | Date & Time | [x] | bo-formula |
| WEEKDAY | Date & Time | [x] | bo-formula |
| WEEKNUM | Date & Time | [x] | bo-formula |
| WORKDAY | Date & Time | [x] | bo-formula |
| WORKDAY.INTL | Date & Time | [x] | bo-formula |
| YEAR | Date & Time | [x] | bo-formula |
| YEARFRAC | Date & Time | [x] | bo-formula |
| BESSELI | Engineering | [x] | bo-formula |
| BESSELJ | Engineering | [x] | bo-formula |
| BESSELK | Engineering | [x] | bo-formula |
| BESSELY | Engineering | [x] | bo-formula |
| BIN2DEC | Engineering | [x] | bo-formula |
| BIN2HEX | Engineering | [x] | bo-formula |
| BIN2OCT | Engineering | [x] | bo-formula |
| BITAND | Engineering | [x] | bo-formula |
| BITLSHIFT | Engineering | [x] | bo-formula |
| BITOR | Engineering | [x] | bo-formula |
| BITRSHIFT | Engineering | [x] | bo-formula |
| BITXOR | Engineering | [x] | bo-formula |
| COMPLEX | Engineering | [x] | bo-formula |
| CONVERT | Engineering | [x] | bo-formula |
| DEC2BIN | Engineering | [x] | bo-formula |
| DEC2HEX | Engineering | [x] | bo-formula |
| DEC2OCT | Engineering | [x] | bo-formula |
| DELTA | Engineering | [x] | bo-formula |
| ERF | Engineering | [x] | bo-formula |
| ERF.PRECISE | Engineering | [x] | bo-formula |
| ERFC | Engineering | [x] | bo-formula |
| ERFC.PRECISE | Engineering | [x] | bo-formula |
| GESTEP | Engineering | [x] | bo-formula |
| HEX2BIN | Engineering | [x] | bo-formula |
| HEX2DEC | Engineering | [x] | bo-formula |
| HEX2OCT | Engineering | [x] | bo-formula |
| IMABS | Engineering | [x] | bo-formula |
| IMAGINARY | Engineering | [x] | bo-formula |
| IMARGUMENT | Engineering | [x] | bo-formula |
| IMCONJUGATE | Engineering | [x] | bo-formula |
| IMCOS | Engineering | [x] | bo-formula |
| IMCOSH | Engineering | [x] | bo-formula |
| IMCOT | Engineering | [x] | bo-formula |
| IMCSC | Engineering | [x] | bo-formula |
| IMCSCH | Engineering | [x] | bo-formula |
| IMDIV | Engineering | [x] | bo-formula |
| IMEXP | Engineering | [x] | bo-formula |
| IMLN | Engineering | [x] | bo-formula |
| IMLOG10 | Engineering | [x] | bo-formula |
| IMLOG2 | Engineering | [x] | bo-formula |
| IMPOWER | Engineering | [x] | bo-formula |
| IMPRODUCT | Engineering | [x] | bo-formula |
| IMREAL | Engineering | [x] | bo-formula |
| IMSEC | Engineering | [x] | bo-formula |
| IMSECH | Engineering | [x] | bo-formula |
| IMSIN | Engineering | [x] | bo-formula |
| IMSINH | Engineering | [x] | bo-formula |
| IMSQRT | Engineering | [x] | bo-formula |
| IMSUB | Engineering | [x] | bo-formula |
| IMSUM | Engineering | [x] | bo-formula |
| IMTAN | Engineering | [x] | bo-formula |
| OCT2BIN | Engineering | [x] | bo-formula |
| OCT2DEC | Engineering | [x] | bo-formula |
| OCT2HEX | Engineering | [x] | bo-formula |
| ACCRINT | Financial | [ ] |  |
| ACCRINTM | Financial | [ ] |  |
| AMORDEGRC | Financial | [ ] |  |
| AMORLINC | Financial | [ ] |  |
| COUPDAYBS | Financial | [ ] |  |
| COUPDAYS | Financial | [ ] |  |
| COUPDAYSNC | Financial | [ ] |  |
| COUPNCD | Financial | [ ] |  |
| COUPNUM | Financial | [ ] |  |
| COUPPCD | Financial | [ ] |  |
| CUMIPMT | Financial | [x] | bo-formula |
| CUMPRINC | Financial | [x] | bo-formula |
| DB | Financial | [x] | bo-formula |
| DDB | Financial | [x] | bo-formula |
| DISC | Financial | [ ] |  |
| DOLLARDE | Financial | [ ] |  |
| DOLLARFR | Financial | [ ] |  |
| DURATION | Financial | [ ] |  |
| EFFECT | Financial | [x] | bo-formula |
| FV | Financial | [x] | bo-formula |
| FVSCHEDULE | Financial | [ ] |  |
| INTRATE | Financial | [ ] |  |
| IPMT | Financial | [x] | bo-formula |
| IRR | Financial | [x] | bo-formula |
| ISPMT | Financial | [ ] |  |
| MDURATION | Financial | [ ] |  |
| MIRR | Financial | [ ] |  |
| NOMINAL | Financial | [x] | bo-formula |
| NPER | Financial | [x] | bo-formula |
| NPV | Financial | [x] | bo-formula |
| ODDFPRICE | Financial | [ ] |  |
| ODDFYIELD | Financial | [ ] |  |
| ODDLPRICE | Financial | [ ] |  |
| ODDLYIELD | Financial | [ ] |  |
| PDURATION | Financial | [ ] |  |
| PMT | Financial | [x] | bo-formula |
| PPMT | Financial | [x] | bo-formula |
| PRICE | Financial | [ ] |  |
| PRICEDISC | Financial | [ ] |  |
| PRICEMAT | Financial | [ ] |  |
| PV | Financial | [x] | bo-formula |
| RATE | Financial | [x] | bo-formula |
| RECEIVED | Financial | [ ] |  |
| RRI | Financial | [ ] |  |
| SLN | Financial | [x] | bo-formula |
| SYD | Financial | [x] | bo-formula |
| TBILLEQ | Financial | [ ] |  |
| TBILLPRICE | Financial | [ ] |  |
| TBILLYIELD | Financial | [ ] |  |
| VDB | Financial | [ ] |  |
| XIRR | Financial | [x] | bo-formula |
| XNPV | Financial | [x] | bo-formula |
| YIELD | Financial | [ ] |  |
| YIELDDISC | Financial | [ ] |  |
| YIELDMAT | Financial | [ ] |  |
| CELL | Information | [x] | bo-formula |
| ERROR.TYPE | Information | [x] | bo-formula |
| INFO | Information | [ ] |  |
| ISBLANK | Information | [x] | bo-formula |
| ISERR | Information | [x] | bo-formula |
| ISERROR | Information | [x] | bo-formula |
| ISEVEN | Information | [x] | bo-formula |
| ISFORMULA | Information | [x] | bo-formula |
| ISLOGICAL | Information | [x] | bo-formula |
| ISNA | Information | [x] | bo-formula |
| ISNONTEXT | Information | [x] | bo-formula |
| ISNUMBER | Information | [x] | bo-formula |
| ISODD | Information | [x] | bo-formula |
| ISOMITTED | Information | [ ] |  |
| ISREF | Information | [x] | bo-formula |
| ISTEXT | Information | [x] | bo-formula |
| N | Information | [x] | bo-formula |
| NA | Information | [x] | bo-formula |
| SHEET | Information | [x] | bo-formula |
| SHEETS | Information | [x] | bo-formula |
| TYPE | Information | [x] | bo-formula |
| AND | Logical | [x] | bo-formula |
| BYCOL | Logical | [x] | bo-formula |
| BYROW | Logical | [x] | bo-formula |
| FALSE | Logical | [x] | bo-formula |
| IF | Logical | [x] | bo-formula |
| IFERROR | Logical | [x] | bo-formula |
| IFNA | Logical | [x] | bo-formula |
| IFS | Logical | [x] | bo-formula |
| LAMBDA | Logical | [x] | bo-formula |
| LET | Logical | [x] | bo-formula |
| MAKEARRAY | Logical | [x] | bo-formula |
| MAP | Logical | [x] | bo-formula |
| NOT | Logical | [x] | bo-formula |
| OR | Logical | [x] | bo-formula |
| REDUCE | Logical | [x] | bo-formula |
| SCAN | Logical | [x] | bo-formula |
| SWITCH | Logical | [x] | bo-formula |
| TRUE | Logical | [x] | bo-formula |
| XOR | Logical | [x] | bo-formula |
| ADDRESS | Lookup & Reference | [ ] |  |
| AREAS | Lookup & Reference | [x] | bo-formula |
| CHOOSE | Lookup & Reference | [x] | bo-formula |
| CHOOSECOLS | Lookup & Reference | [x] | bo-formula |
| CHOOSEROWS | Lookup & Reference | [x] | bo-formula |
| COLUMN | Lookup & Reference | [x] | bo-formula |
| COLUMNS | Lookup & Reference | [x] | bo-formula |
| DROP | Lookup & Reference | [x] | bo-formula |
| EXPAND | Lookup & Reference | [ ] |  |
| FILTER | Lookup & Reference | [x] | bo-formula |
| FORMULATEXT | Lookup & Reference | [x] | bo-formula |
| GETPIVOTDATA | Lookup & Reference | [ ] |  |
| GROUPBY | Lookup & Reference | [ ] |  |
| HLOOKUP | Lookup & Reference | [x] | bo-formula |
| HSTACK | Lookup & Reference | [x] | bo-formula |
| HYPERLINK | Lookup & Reference | [ ] |  |
| IMAGE | Lookup & Reference | [ ] |  |
| INDEX | Lookup & Reference | [x] | bo-formula |
| INDIRECT | Lookup & Reference | [x] | bo-formula |
| LOOKUP | Lookup & Reference | [x] | bo-formula |
| MATCH | Lookup & Reference | [x] | bo-formula |
| OFFSET | Lookup & Reference | [x] | bo-formula |
| PIVOTBY | Lookup & Reference | [ ] |  |
| ROW | Lookup & Reference | [x] | bo-formula |
| ROWS | Lookup & Reference | [x] | bo-formula |
| RTD | Lookup & Reference | [ ] |  |
| SORT | Lookup & Reference | [x] | bo-formula |
| SORTBY | Lookup & Reference | [x] | bo-formula |
| TAKE | Lookup & Reference | [x] | bo-formula |
| TOCOL | Lookup & Reference | [ ] |  |
| TOROW | Lookup & Reference | [ ] |  |
| TRANSPOSE | Lookup & Reference | [x] | bo-formula |
| TRIMRANGE | Lookup & Reference | [ ] |  |
| UNIQUE | Lookup & Reference | [x] | bo-formula |
| VLOOKUP | Lookup & Reference | [x] | bo-formula |
| VSTACK | Lookup & Reference | [x] | bo-formula |
| WRAPCOLS | Lookup & Reference | [ ] |  |
| WRAPROWS | Lookup & Reference | [ ] |  |
| XLOOKUP | Lookup & Reference | [x] | bo-formula |
| XMATCH | Lookup & Reference | [x] | bo-formula |
| ABS | Math & Trig | [x] | bo-formula |
| ACOS | Math & Trig | [x] | bo-formula |
| ACOSH | Math & Trig | [x] | bo-formula |
| ACOT | Math & Trig | [x] | bo-formula |
| ACOTH | Math & Trig | [x] | bo-formula |
| AGGREGATE | Math & Trig | [x] | bo-formula |
| ARABIC | Math & Trig | [ ] |  |
| ASIN | Math & Trig | [x] | bo-formula |
| ASINH | Math & Trig | [x] | bo-formula |
| ATAN | Math & Trig | [x] | bo-formula |
| ATAN2 | Math & Trig | [x] | bo-formula |
| ATANH | Math & Trig | [x] | bo-formula |
| BASE | Math & Trig | [ ] |  |
| CEILING.MATH | Math & Trig | [x] | bo-formula |
| CEILING.PRECISE | Math & Trig | [x] | bo-formula |
| COMBIN | Math & Trig | [x] | bo-formula |
| COMBINA | Math & Trig | [x] | bo-formula |
| COS | Math & Trig | [x] | bo-formula |
| COSH | Math & Trig | [x] | bo-formula |
| COT | Math & Trig | [x] | bo-formula |
| COTH | Math & Trig | [x] | bo-formula |
| CSC | Math & Trig | [x] | bo-formula |
| CSCH | Math & Trig | [x] | bo-formula |
| DECIMAL | Math & Trig | [ ] |  |
| DEGREES | Math & Trig | [x] | bo-formula |
| EVEN | Math & Trig | [x] | bo-formula |
| EXP | Math & Trig | [x] | bo-formula |
| FACT | Math & Trig | [x] | bo-formula |
| FACTDOUBLE | Math & Trig | [x] | bo-formula |
| FLOOR.MATH | Math & Trig | [x] | bo-formula |
| FLOOR.PRECISE | Math & Trig | [x] | bo-formula |
| GCD | Math & Trig | [x] | bo-formula |
| INT | Math & Trig | [x] | bo-formula |
| ISO.CEILING | Math & Trig | [x] | bo-formula |
| LCM | Math & Trig | [x] | bo-formula |
| LN | Math & Trig | [x] | bo-formula |
| LOG | Math & Trig | [x] | bo-formula |
| LOG10 | Math & Trig | [x] | bo-formula |
| MDETERM | Math & Trig | [ ] |  |
| MINVERSE | Math & Trig | [ ] |  |
| MMULT | Math & Trig | [ ] |  |
| MOD | Math & Trig | [x] | bo-formula |
| MROUND | Math & Trig | [x] | bo-formula |
| MULTINOMIAL | Math & Trig | [x] | bo-formula |
| MUNIT | Math & Trig | [ ] |  |
| ODD | Math & Trig | [x] | bo-formula |
| PERCENTOF | Math & Trig | [ ] |  |
| PI | Math & Trig | [x] | bo-formula |
| POWER | Math & Trig | [x] | bo-formula |
| PRODUCT | Math & Trig | [x] | bo-formula |
| QUOTIENT | Math & Trig | [x] | bo-formula |
| RADIANS | Math & Trig | [x] | bo-formula |
| RAND | Math & Trig | [x] | bo-formula |
| RANDARRAY | Math & Trig | [x] | bo-formula |
| RANDBETWEEN | Math & Trig | [x] | bo-formula |
| ROMAN | Math & Trig | [ ] |  |
| ROUND | Math & Trig | [x] | bo-formula |
| ROUNDDOWN | Math & Trig | [x] | bo-formula |
| ROUNDUP | Math & Trig | [x] | bo-formula |
| SEC | Math & Trig | [x] | bo-formula |
| SECH | Math & Trig | [x] | bo-formula |
| SEQUENCE | Math & Trig | [x] | bo-formula |
| SERIESSUM | Math & Trig | [ ] |  |
| SIGN | Math & Trig | [x] | bo-formula |
| SIN | Math & Trig | [x] | bo-formula |
| SINH | Math & Trig | [x] | bo-formula |
| SQRT | Math & Trig | [x] | bo-formula |
| SQRTPI | Math & Trig | [x] | bo-formula |
| SUBTOTAL | Math & Trig | [x] | bo-formula |
| SUM | Math & Trig | [x] | bo-formula |
| SUMIF | Math & Trig | [x] | bo-formula |
| SUMIFS | Math & Trig | [x] | bo-formula |
| SUMPRODUCT | Math & Trig | [x] | bo-formula |
| SUMSQ | Math & Trig | [x] | bo-formula |
| SUMX2MY2 | Math & Trig | [x] | bo-formula |
| SUMX2PY2 | Math & Trig | [x] | bo-formula |
| SUMXMY2 | Math & Trig | [x] | bo-formula |
| TAN | Math & Trig | [x] | bo-formula |
| TANH | Math & Trig | [x] | bo-formula |
| TRUNC | Math & Trig | [x] | bo-formula |
| AVEDEV | Statistical | [x] | bo-formula |
| AVERAGE | Statistical | [x] | bo-formula |
| AVERAGEA | Statistical | [x] | bo-formula |
| AVERAGEIF | Statistical | [x] | bo-formula |
| AVERAGEIFS | Statistical | [x] | bo-formula |
| BETA.DIST | Statistical | [ ] |  |
| BETA.INV | Statistical | [ ] |  |
| BINOM.DIST | Statistical | [ ] |  |
| BINOM.DIST.RANGE | Statistical | [ ] |  |
| BINOM.INV | Statistical | [ ] |  |
| CHISQ.DIST | Statistical | [ ] |  |
| CHISQ.DIST.RT | Statistical | [ ] |  |
| CHISQ.INV | Statistical | [ ] |  |
| CHISQ.INV.RT | Statistical | [ ] |  |
| CHISQ.TEST | Statistical | [ ] |  |
| CONFIDENCE.NORM | Statistical | [ ] |  |
| CONFIDENCE.T | Statistical | [ ] |  |
| CORREL | Statistical | [x] | bo-formula |
| COUNT | Statistical | [x] | bo-formula |
| COUNTA | Statistical | [x] | bo-formula |
| COUNTBLANK | Statistical | [x] | bo-formula |
| COUNTIF | Statistical | [x] | bo-formula |
| COUNTIFS | Statistical | [x] | bo-formula |
| COVARIANCE.P | Statistical | [x] | bo-formula |
| COVARIANCE.S | Statistical | [x] | bo-formula |
| DEVSQ | Statistical | [x] | bo-formula |
| EXPON.DIST | Statistical | [ ] |  |
| F.DIST | Statistical | [ ] |  |
| F.DIST.RT | Statistical | [ ] |  |
| F.INV | Statistical | [ ] |  |
| F.INV.RT | Statistical | [ ] |  |
| F.TEST | Statistical | [ ] |  |
| FISHER | Statistical | [ ] |  |
| FISHERINV | Statistical | [ ] |  |
| FORECAST.ETS | Statistical | [ ] |  |
| FORECAST.ETS.CONFINT | Statistical | [ ] |  |
| FORECAST.ETS.SEASONALITY | Statistical | [ ] |  |
| FORECAST.ETS.STAT | Statistical | [ ] |  |
| FORECAST.LINEAR | Statistical | [x] | bo-formula |
| FREQUENCY | Statistical | [ ] |  |
| GAMMA | Statistical | [ ] |  |
| GAMMA.DIST | Statistical | [ ] |  |
| GAMMA.INV | Statistical | [ ] |  |
| GAMMALN | Statistical | [ ] |  |
| GAMMALN.PRECISE | Statistical | [ ] |  |
| GAUSS | Statistical | [ ] |  |
| GEOMEAN | Statistical | [x] | bo-formula |
| GROWTH | Statistical | [ ] |  |
| HARMEAN | Statistical | [x] | bo-formula |
| HYPGEOM.DIST | Statistical | [ ] |  |
| INTERCEPT | Statistical | [x] | bo-formula |
| KURT | Statistical | [ ] |  |
| LARGE | Statistical | [x] | bo-formula |
| LINEST | Statistical | [ ] |  |
| LOGEST | Statistical | [ ] |  |
| LOGNORM.DIST | Statistical | [ ] |  |
| LOGNORM.INV | Statistical | [ ] |  |
| MAX | Statistical | [x] | bo-formula |
| MAXA | Statistical | [x] | bo-formula |
| MAXIFS | Statistical | [x] | bo-formula |
| MEDIAN | Statistical | [x] | bo-formula |
| MIN | Statistical | [x] | bo-formula |
| MINA | Statistical | [x] | bo-formula |
| MINIFS | Statistical | [x] | bo-formula |
| MODE.MULT | Statistical | [ ] |  |
| MODE.SNGL | Statistical | [x] | bo-formula |
| NEGBINOM.DIST | Statistical | [ ] |  |
| NORM.DIST | Statistical | [x] | bo-formula |
| NORM.INV | Statistical | [x] | bo-formula |
| NORM.S.DIST | Statistical | [x] | bo-formula |
| NORM.S.INV | Statistical | [x] | bo-formula |
| PEARSON | Statistical | [x] | bo-formula |
| PERCENTILE.EXC | Statistical | [x] | bo-formula |
| PERCENTILE.INC | Statistical | [x] | bo-formula |
| PERCENTRANK.EXC | Statistical | [ ] |  |
| PERCENTRANK.INC | Statistical | [ ] |  |
| PERMUT | Statistical | [x] | bo-formula |
| PERMUTATIONA | Statistical | [x] | bo-formula |
| PHI | Statistical | [ ] |  |
| POISSON.DIST | Statistical | [ ] |  |
| PROB | Statistical | [ ] |  |
| QUARTILE.EXC | Statistical | [x] | bo-formula |
| QUARTILE.INC | Statistical | [x] | bo-formula |
| RANK.AVG | Statistical | [x] | bo-formula |
| RANK.EQ | Statistical | [x] | bo-formula |
| RSQ | Statistical | [x] | bo-formula |
| SKEW | Statistical | [ ] |  |
| SKEW.P | Statistical | [ ] |  |
| SLOPE | Statistical | [x] | bo-formula |
| SMALL | Statistical | [x] | bo-formula |
| STANDARDIZE | Statistical | [x] | bo-formula |
| STDEV.P | Statistical | [x] | bo-formula |
| STDEV.S | Statistical | [x] | bo-formula |
| STDEVA | Statistical | [x] | bo-formula |
| STDEVPA | Statistical | [ ] |  |
| STEYX | Statistical | [ ] |  |
| T.DIST | Statistical | [ ] |  |
| T.DIST.2T | Statistical | [ ] |  |
| T.DIST.RT | Statistical | [ ] |  |
| T.INV | Statistical | [ ] |  |
| T.INV.2T | Statistical | [ ] |  |
| T.TEST | Statistical | [ ] |  |
| TREND | Statistical | [ ] |  |
| TRIMMEAN | Statistical | [ ] |  |
| VAR.P | Statistical | [x] | bo-formula |
| VAR.S | Statistical | [x] | bo-formula |
| VARA | Statistical | [x] | bo-formula |
| VARPA | Statistical | [ ] |  |
| WEIBULL.DIST | Statistical | [ ] |  |
| Z.TEST | Statistical | [ ] |  |
| ARRAYTOTEXT | Text | [ ] |  |
| ASC | Text | [ ] |  |
| BAHTTEXT | Text | [ ] |  |
| CHAR | Text | [x] | bo-formula |
| CLEAN | Text | [x] | bo-formula |
| CODE | Text | [x] | bo-formula |
| CONCAT | Text | [x] | bo-formula |
| DBCS | Text | [ ] |  |
| DOLLAR | Text | [ ] |  |
| EXACT | Text | [x] | bo-formula |
| FIND | Text | [x] | bo-formula |
| FINDB | Text | [ ] |  |
| FIXED | Text | [x] | bo-formula |
| LEFT | Text | [x] | bo-formula |
| LEFTB | Text | [ ] |  |
| LEN | Text | [x] | bo-formula |
| LENB | Text | [ ] |  |
| LOWER | Text | [x] | bo-formula |
| MID | Text | [x] | bo-formula |
| MIDB | Text | [ ] |  |
| NUMBERVALUE | Text | [x] | bo-formula |
| PHONETIC | Text | [ ] |  |
| PROPER | Text | [x] | bo-formula |
| REGEXEXTRACT | Text | [ ] |  |
| REGEXREPLACE | Text | [ ] |  |
| REGEXTEST | Text | [ ] |  |
| REPLACE | Text | [x] | bo-formula |
| REPLACEB | Text | [ ] |  |
| REPT | Text | [x] | bo-formula |
| RIGHT | Text | [x] | bo-formula |
| RIGHTB | Text | [ ] |  |
| SEARCH | Text | [x] | bo-formula |
| SEARCHB | Text | [ ] |  |
| SUBSTITUTE | Text | [x] | bo-formula |
| T | Text | [x] | bo-formula |
| TEXT | Text | [x] | bo-formula |
| TEXTAFTER | Text | [x] | bo-formula |
| TEXTBEFORE | Text | [x] | bo-formula |
| TEXTJOIN | Text | [x] | bo-formula |
| TEXTSPLIT | Text | [x] | bo-formula |
| TRIM | Text | [x] | bo-formula |
| UNICHAR | Text | [x] | bo-formula |
| UNICODE | Text | [x] | bo-formula |
| UPPER | Text | [x] | bo-formula |
| VALUE | Text | [x] | bo-formula |
| VALUETOTEXT | Text | [ ] |  |
| ENCODEURL | Web | [ ] |  |
| FILTERXML | Web | [ ] |  |
| WEBSERVICE | Web | [ ] |  |
| BO.DOCSO | BlueOffice | [ ] |  |
