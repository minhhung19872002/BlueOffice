# Preset text warp definitions, ECMA-376 Part 1 (5th ed.), Annex D presetTextWarpDefinitions.xml
# (§20.1.9.19 prstTxWarp, §20.1.10.76 ST_TextShapeType), converted mechanically to the
# line format documented in presets.rs. ahLst is omitted. Do not edit by hand:
# regenerate from the annex.

@textArchDown
av adj val 0
gd adval pin 0 adj 21599999
gd v1 +- 10800000 0 adval
gd v2 +- 32400000 0 adval
gd nv1 +- 0 0 v1
gd stAng ?: nv1 v2 v1
gd w1 +- 5400000 0 adval
gd w2 +- 16200000 0 adval
gd d1 +- adval 0 stAng
gd d2 +- d1 0 21600000
gd v3 +- 0 0 10800000
gd c2 ?: w2 d1 d2
gd c1 ?: v1 d2 c2
gd c0 ?: w1 d1 c1
gd swAng ?: stAng c0 v3
gd wt1 sin wd2 adj
gd ht1 cos hd2 adj
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd wt2 sin wd2 stAng
gd ht2 cos hd2 stAng
gd dx2 cat2 wd2 ht2 wt2
gd dy2 sat2 hd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
path
M x2 y2; A wd2 hd2 stAng swAng

@textArchDownPour
av adj1 val 0
av adj2 val 25000
gd adval pin 0 adj1 21599999
gd v1 +- 10800000 0 adval
gd v2 +- 32400000 0 adval
gd nv1 +- 0 0 v1
gd stAng ?: nv1 v2 v1
gd w1 +- 5400000 0 adval
gd w2 +- 16200000 0 adval
gd d1 +- adval 0 stAng
gd d2 +- d1 0 21600000
gd v3 +- 0 0 10800000
gd c2 ?: w2 d1 d2
gd c1 ?: v1 d2 c2
gd c0 ?: w1 d1 c1
gd swAng ?: stAng c0 v3
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd adval2 pin 0 adj2 99000
gd ratio */ adval2 1 100000
gd iwd2 */ wd2 ratio 1
gd ihd2 */ hd2 ratio 1
gd wt2 sin iwd2 adval
gd ht2 cos ihd2 adval
gd dx2 cat2 iwd2 ht2 wt2
gd dy2 sat2 ihd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd wt3 sin iwd2 stAng
gd ht3 cos ihd2 stAng
gd dx3 cat2 iwd2 ht3 wt3
gd dy3 sat2 ihd2 ht3 wt3
gd x3 +- hc dx3 0
gd y3 +- vc dy3 0
path
M x3 y3; A iwd2 ihd2 stAng swAng
path
M x1 y1; A wd2 hd2 stAng swAng

@textArchUp
av adj val cd2
gd adval pin 0 adj 21599999
gd v1 +- 10800000 0 adval
gd v2 +- 32400000 0 adval
gd end ?: v1 v1 v2
gd w1 +- 5400000 0 adval
gd w2 +- 16200000 0 adval
gd d1 +- end 0 adval
gd d2 +- 21600000 d1 0
gd c2 ?: w2 d1 d2
gd c1 ?: v1 d2 c2
gd swAng ?: w1 d1 c1
gd wt1 sin wd2 adj
gd ht1 cos hd2 adj
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path
M x1 y1; A wd2 hd2 adval swAng

@textArchUpPour
av adj1 val cd2
av adj2 val 50000
gd adval pin 0 adj1 21599999
gd v1 +- 10800000 0 adval
gd v2 +- 32400000 0 adval
gd end ?: v1 v1 v2
gd w1 +- 5400000 0 adval
gd w2 +- 16200000 0 adval
gd d1 +- end 0 adval
gd d2 +- 21600000 d1 0
gd c2 ?: w2 d1 d2
gd c1 ?: v1 d2 c2
gd swAng ?: w1 d1 c1
gd wt1 sin wd2 adval
gd ht1 cos hd2 adval
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd adval2 pin 0 adj2 99000
gd ratio */ adval2 1 100000
gd iwd2 */ wd2 ratio 1
gd ihd2 */ hd2 ratio 1
gd wt2 sin iwd2 adval
gd ht2 cos ihd2 adval
gd dx2 cat2 iwd2 ht2 wt2
gd dy2 sat2 ihd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
path
M x1 y1; A wd2 hd2 adval swAng
path
M x2 y2; A iwd2 ihd2 adval swAng

@textButton
av adj val 10800000
gd adval pin 0 adj 21599999
gd bot +- 5400000 0 adval
gd lef +- 10800000 0 adval
gd top +- 16200000 0 adval
gd rig +- 21600000 0 adval
gd c3 ?: top adval 0
gd c2 ?: lef 10800000 c3
gd c1 ?: bot rig c2
gd stAng ?: adval c1 0
gd w1 +- 21600000 0 stAng
gd stAngB ?: stAng w1 0
gd td1 */ bot 2 1
gd td2 */ top 2 1
gd ntd2 +- 0 0 td2
gd w2 +- 0 0 10800000
gd c6 ?: top ntd2 w2
gd c5 ?: lef 10800000 c6
gd c4 ?: bot td1 c5
gd v1 ?: adval c4 10800000
gd swAngT +- 0 0 v1
gd stT ?: lef stAngB stAng
gd stB ?: lef stAng stAngB
gd swT ?: lef v1 swAngT
gd swB ?: lef swAngT v1
gd wt1 sin wd2 stT
gd ht1 cos hd2 stT
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd wt2 sin wd2 stB
gd ht2 cos hd2 stB
gd dx2 cat2 wd2 ht2 wt2
gd dy2 sat2 hd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd wt3 sin wd2 adj
gd ht3 cos hd2 adj
gd dx3 cat2 wd2 ht3 wt3
gd dy3 sat2 hd2 ht3 wt3
gd x3 +- hc dx3 0
gd y3 +- vc dy3 0
path
M x1 y1; A wd2 hd2 stT swT
path
M l vc; L r vc
path
M x2 y2; A wd2 hd2 stB swB

@textButtonPour
av adj1 val cd2
av adj2 val 50000
gd adval pin 0 adj1 21599999
gd bot +- 5400000 0 adval
gd lef +- 10800000 0 adval
gd top +- 16200000 0 adval
gd rig +- 21600000 0 adval
gd c3 ?: top adval 0
gd c2 ?: lef 10800000 c3
gd c1 ?: bot rig c2
gd stAng ?: adval c1 0
gd w1 +- 21600000 0 stAng
gd stAngB ?: stAng w1 0
gd td1 */ bot 2 1
gd td2 */ top 2 1
gd ntd2 +- 0 0 td2
gd w2 +- 0 0 10800000
gd c6 ?: top ntd2 w2
gd c5 ?: lef 10800000 c6
gd c4 ?: bot td1 c5
gd v1 ?: adval c4 10800000
gd swAngT +- 0 0 v1
gd stT ?: lef stAngB stAng
gd stB ?: lef stAng stAngB
gd swT ?: lef v1 swAngT
gd swB ?: lef swAngT v1
gd wt1 sin wd2 stT
gd ht1 cos hd2 stT
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd wt6 sin wd2 stB
gd ht6 cos hd2 stB
gd dx6 cat2 wd2 ht6 wt6
gd dy6 sat2 hd2 ht6 wt6
gd x6 +- hc dx6 0
gd y6 +- vc dy6 0
gd adval2 pin 40000 adj2 99000
gd ratio */ adval2 1 100000
gd iwd2 */ wd2 ratio 1
gd ihd2 */ hd2 ratio 1
gd wt2 sin iwd2 stT
gd ht2 cos ihd2 stT
gd dx2 cat2 iwd2 ht2 wt2
gd dy2 sat2 ihd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd wt5 sin iwd2 stB
gd ht5 cos ihd2 stB
gd dx5 cat2 iwd2 ht5 wt5
gd dy5 sat2 ihd2 ht5 wt5
gd x5 +- hc dx5 0
gd y5 +- vc dy5 0
gd d1 +- hd2 0 ihd2
gd d12 */ d1 1 2
gd yu +- vc 0 d12
gd yd +- vc d12 0
gd v1 */ d12 d12 1
gd v2 */ ihd2 ihd2 1
gd v3 */ v1 1 v2
gd v4 +- 1 0 v3
gd v5 */ iwd2 iwd2 1
gd v6 */ v4 v5 1
gd v7 sqrt v6
gd xl +- hc 0 v7
gd xr +- hc v7 0
gd wtadj sin iwd2 adj1
gd htadj cos ihd2 adj1
gd dxadj cat2 iwd2 htadj wtadj
gd dyadj sat2 ihd2 htadj wtadj
gd xadj +- hc dxadj 0
gd yadj +- vc dyadj 0
path
M x1 y1; A wd2 hd2 stT swT
path
M x2 y2; A iwd2 ihd2 stT swT
path
M xl yu; L xr yu
path
M xl yd; L xr yd
path
M x5 y5; A iwd2 ihd2 stB swB
path
M x6 y6; A wd2 hd2 stB swB

@textCanDown
av adj val 14286
gd a pin 0 adj 33333
gd dy */ a h 100000
gd y0 +- t dy 0
gd y1 +- b 0 dy
gd ncd2 */ cd2 -1 1
path
M l t; A wd2 dy cd2 ncd2
path
M l y1; A wd2 dy cd2 ncd2

@textCanUp
av adj val 85714
gd a pin 66667 adj 100000
gd dy1 */ a h 100000
gd dy +- h 0 dy1
gd y0 +- t dy1 0
gd y1 +- t dy 0
path
M l y1; A wd2 dy cd2 cd2
path
M l b; A wd2 dy cd2 cd2

@textCascadeDown
av adj val 44444
gd a pin 28570 adj 100000
gd dy */ a h 100000
gd y1 +- t dy 0
gd dy2 +- h 0 dy
gd dy3 */ dy2 1 4
gd y2 +- t dy3 0
path
M l t; L r y2
path
M l y1; L r b

@textCascadeUp
av adj val 44444
gd a pin 28570 adj 100000
gd dy */ a h 100000
gd y1 +- t dy 0
gd dy2 +- h 0 dy
gd dy3 */ dy2 1 4
gd y2 +- t dy3 0
path
M l y2; L r t
path
M l b; L r y1

@textChevron
av adj val 25000
gd a pin 0 adj 50000
gd y */ a h 100000
gd y1 +- t b y
path
M l y; L hc t; L r y
path
M l b; L hc y1; L r b

@textChevronInverted
av adj val 75000
gd a pin 50000 adj 100000
gd y */ a h 100000
gd y1 +- b 0 y
path
M l t; L hc y1; L r t
path
M l y; L hc b; L r y

@textCircle
av adj val 10800000
gd adval pin 0 adj 21599999
gd d0 +- adval 0 10800000
gd d1 +- 10800000 0 adval
gd d2 +- 21600000 0 adval
gd d3 ?: d1 d1 10799999
gd d4 ?: d0 d2 d3
gd swAng */ d4 2 1
gd wt1 sin wd2 adj
gd ht1 cos hd2 adj
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path
M x1 y1; A wd2 hd2 adval swAng

@textCirclePour
av adj1 val cd2
av adj2 val 50000
gd adval pin 0 adj1 21599999
gd d0 +- adval 0 10800000
gd d1 +- 10800000 0 adval
gd d2 +- 21600000 0 adval
gd d3 ?: d1 d1 10799999
gd d4 ?: d0 d2 d3
gd swAng */ d4 2 1
gd wt1 sin wd2 adval
gd ht1 cos hd2 adval
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd adval2 pin 0 adj2 99000
gd ratio */ adval2 1 100000
gd iwd2 */ wd2 ratio 1
gd ihd2 */ hd2 ratio 1
gd wt2 sin iwd2 adval
gd ht2 cos ihd2 adval
gd dx2 cat2 iwd2 ht2 wt2
gd dy2 sat2 ihd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
path
M x1 y1; A wd2 hd2 adval swAng
path
M x2 y2; A iwd2 ihd2 adval swAng

@textCurveDown
av adj val 45977
gd a pin 0 adj 56338
gd dy */ a h 100000
gd gd1 */ dy 3 4
gd gd2 */ dy 5 4
gd gd3 */ dy 3 8
gd gd4 */ dy 1 8
gd gd5 +- h 0 gd3
gd gd6 +- gd4 h 0
gd y0 +- t dy 0
gd y1 +- t gd1 0
gd y2 +- t gd2 0
gd y3 +- t gd3 0
gd y4 +- t gd4 0
gd y5 +- t gd5 0
gd y6 +- t gd6 0
gd x1 +- l wd3 0
gd x2 +- r 0 wd3
path
M l t; C x1 y1 x2 y2 r y0
path
M l y5; C x1 y6 x2 y6 r y5

@textCurveUp
av adj val 45977
gd a pin 0 adj 56338
gd dy */ a h 100000
gd gd1 */ dy 3 4
gd gd2 */ dy 5 4
gd gd3 */ dy 3 8
gd gd4 */ dy 1 8
gd gd5 +- h 0 gd3
gd gd6 +- gd4 h 0
gd y0 +- t dy 0
gd y1 +- t gd1 0
gd y2 +- t gd2 0
gd y3 +- t gd3 0
gd y4 +- t gd4 0
gd y5 +- t gd5 0
gd y6 +- t gd6 0
gd x1 +- l wd3 0
gd x2 +- r 0 wd3
path
M l y0; C x1 y2 x2 y1 r t
path
M l y5; C x1 y6 x2 y6 r y5

@textDeflate
av adj val 18750
gd a pin 0 adj 37500
gd dy */ a ss 100000
gd gd0 */ dy 4 3
gd gd1 +- h 0 gd0
gd adjY +- t dy 0
gd y0 +- t gd0 0
gd y1 +- t gd1 0
gd x0 +- l wd3 0
gd x1 +- r 0 wd3
path
M l t; C x0 y0 x1 y0 r t
path
M l b; C x0 y1 x1 y1 r b

@textDeflateBottom
av adj val 50000
gd a pin 6250 adj 100000
gd dy */ a ss 100000
gd dy2 +- h 0 dy
gd y1 +- t dy 0
gd cp +- y1 0 dy2
path
M l t; L r t
path
M l b; Q hc cp r b

@textDeflateInflate
av adj val 35000
gd a pin 5000 adj 95000
gd dy */ a h 100000
gd del */ h 5 100
gd dh1 */ h 45 100
gd dh2 */ h 55 100
gd yh +- dy 0 del
gd yl +- dy del 0
gd y3 +- yh yh dh1
gd y4 +- yl yl dh2
path
M l t; L r t
path
M l dh1; Q hc y3 r dh1
path
M l dh2; Q hc y4 r dh2
path
M l b; L r b

@textDeflateInflateDeflate
av adj val 25000
gd a pin 3000 adj 47000
gd dy */ a h 100000
gd del */ h 3 100
gd ey1 */ h 30 100
gd ey2 */ h 36 100
gd ey3 */ h 63 100
gd ey4 */ h 70 100
gd by +- b 0 dy
gd yh1 +- dy 0 del
gd yl1 +- dy del 0
gd yh2 +- by 0 del
gd yl2 +- by del 0
gd y1 +- yh1 yh1 ey1
gd y2 +- yl1 yl1 ey2
gd y3 +- yh2 yh2 ey3
gd y4 +- yl2 yl2 ey4
path
M l t; L r t
path
M l ey1; Q hc y1 r ey1
path
M l ey2; Q hc y2 r ey2
path
M l ey3; Q hc y3 r ey3
path
M l ey4; Q hc y4 r ey4
path
M l b; L r b

@textDeflateTop
av adj val 50000
gd a pin 0 adj 93750
gd dy */ a h 100000
gd y1 +- t dy 0
gd cp +- y1 dy 0
path
M l t; Q hc cp r t
path
M l b; L r b

@textDoubleWave1
av adj1 val 6250
av adj2 val 0
gd a1 pin 0 adj1 12500
gd a2 pin -10000 adj2 10000
gd y1 */ h a1 100000
gd dy2 */ y1 10 3
gd y2 +- y1 0 dy2
gd y3 +- y1 dy2 0
gd y4 +- b 0 y1
gd y5 +- y4 0 dy2
gd y6 +- y4 dy2 0
gd of */ w a2 100000
gd of2 */ w a2 50000
gd x1 abs of
gd dx2 ?: of2 0 of2
gd x2 +- l 0 dx2
gd dx8 ?: of2 of2 0
gd x8 +- r 0 dx8
gd dx3 +/ dx2 x8 6
gd x3 +- x2 dx3 0
gd dx4 +/ dx2 x8 3
gd x4 +- x2 dx4 0
gd x5 +/ x2 x8 2
gd x6 +- x5 dx3 0
gd x7 +/ x6 x8 2
gd x9 +- l dx8 0
gd x15 +- r dx2 0
gd x10 +- x9 dx3 0
gd x11 +- x9 dx4 0
gd x12 +/ x9 x15 2
gd x13 +- x12 dx3 0
gd x14 +/ x13 x15 2
gd x16 +- r 0 x1
gd xAdj +- hc of 0
path
M x2 y1; C x3 y2 x4 y3 x5 y1; C x6 y2 x7 y3 x8 y1
path
M x9 y4; C x10 y5 x11 y6 x12 y4; C x13 y5 x14 y6 x15 y4

@textFadeDown
av adj val 33333
gd a pin 0 adj 49999
gd dx */ a w 100000
gd x1 +- l dx 0
gd x2 +- r 0 dx
path
M l t; L r t
path
M x1 b; L x2 b

@textFadeLeft
av adj val 33333
gd a pin 0 adj 49999
gd dy */ a h 100000
gd y1 +- t dy 0
gd y2 +- b 0 dy
path
M l y1; L r t
path
M l y2; L r b

@textFadeRight
av adj val 33333
gd a pin 0 adj 49999
gd dy */ a h 100000
gd y1 +- t dy 0
gd y2 +- b 0 dy
path
M l t; L r y1
path
M l b; L r y2

@textFadeUp
av adj val 33333
gd a pin 0 adj 49999
gd dx */ a w 100000
gd x1 +- l dx 0
gd x2 +- r 0 dx
path
M x1 t; L x2 t
path
M l b; L r b

@textInflate
av adj val 18750
gd a pin 0 adj 20000
gd dy */ a h 100000
gd gd */ dy 1 3
gd gd0 +- 0 0 gd
gd gd1 +- h 0 gd0
gd ty +- t dy 0
gd by +- b 0 dy
gd y0 +- t gd0 0
gd y1 +- t gd1 0
gd x0 +- l wd3 0
gd x1 +- r 0 wd3
path
M l ty; C x0 y0 x1 y0 r ty
path
M l by; C x0 y1 x1 y1 r by

@textInflateBottom
av adj val 60000
gd a pin 60000 adj 100000
gd dy */ a h 100000
gd ty +- t dy 0
path
M l t; L r t
path
M l ty; Q hc b r ty

@textInflateTop
av adj val 40000
gd a pin 0 adj 50000
gd dy */ a h 100000
gd ty +- t dy 0
path
M l ty; Q hc t r ty
path
M l b; L r b

@textPlain
av adj val 50000
gd a pin 30000 adj 70000
gd mid */ a w 100000
gd midDir +- mid 0 hc
gd dl +- mid 0 l
gd dr +- r 0 mid
gd dl2 */ dl 2 1
gd dr2 */ dr 2 1
gd dx ?: midDir dr2 dl2
gd xr +- l dx 0
gd xl +- r 0 dx
gd tlx ?: midDir l xl
gd trx ?: midDir xr r
gd blx ?: midDir xl l
gd brx ?: midDir r xr
path
M tlx t; L trx t
path
M blx b; L brx b

@textRingInside
av adj val 60000
gd a pin 50000 adj 99000
gd dy */ a h 100000
gd y +- t dy 0
gd r */ dy 1 2
gd y1 +- t r 0
gd y2 +- b 0 r
path
M l y1; A wd2 r 10800000 21599999
path
M l y2; A wd2 r 10800000 21599999

@textRingOutside
av adj val 60000
gd a pin 50000 adj 99000
gd dy */ a h 100000
gd y +- t dy 0
gd r */ dy 1 2
gd y1 +- t r 0
gd y2 +- b 0 r
path
M l y1; A wd2 r 10800000 -21599999
path
M l y2; A wd2 r 10800000 -21599999

@textSlantDown
av adj val 44445
gd a pin 28569 adj 100000
gd dy */ a h 100000
gd y1 +- t dy 0
gd y2 +- b 0 dy
path
M l t; L r y2
path
M l y1; L r b

@textSlantUp
av adj val 55555
gd a pin 0 adj 71431
gd dy */ a h 100000
gd y1 +- t dy 0
gd y2 +- b 0 dy
path
M l y1; L r t
path
M l b; L r y2

@textStop
av adj val 25000
gd a pin 14286 adj 50000
gd dx */ w 1 3
gd dy */ a h 100000
gd x1 +- l dx 0
gd x2 +- r 0 dx
gd y1 +- t dy 0
gd y2 +- b 0 dy
path
M l y1; L x1 t; L x2 t; L r y1
path
M l y2; L x1 b; L x2 b; L r y2

@textTriangle
av adj val 50000
gd a pin 0 adj 100000
gd y */ a h 100000
path
M l y; L hc t; L r y
path
M l b; L r b

@textTriangleInverted
av adj val 50000
gd a pin 0 adj 100000
gd y */ a h 100000
path
M l t; L r t
path
M l y; L hc b; L r y

@textWave1
av adj1 val 12500
av adj2 val 0
gd a1 pin 0 adj1 20000
gd a2 pin -10000 adj2 10000
gd y1 */ h a1 100000
gd dy2 */ y1 10 3
gd y2 +- y1 0 dy2
gd y3 +- y1 dy2 0
gd y4 +- b 0 y1
gd y5 +- y4 0 dy2
gd y6 +- y4 dy2 0
gd of */ w a2 100000
gd of2 */ w a2 50000
gd x1 abs of
gd dx2 ?: of2 0 of2
gd x2 +- l 0 dx2
gd dx5 ?: of2 of2 0
gd x5 +- r 0 dx5
gd dx3 +/ dx2 x5 3
gd x3 +- x2 dx3 0
gd x4 +/ x3 x5 2
gd x6 +- l dx5 0
gd x10 +- r dx2 0
gd x7 +- x6 dx3 0
gd x8 +/ x7 x10 2
gd x9 +- r 0 x1
gd xAdj +- hc of 0
path
M x2 y1; C x3 y2 x4 y3 x5 y1
path
M x6 y4; C x7 y5 x8 y6 x10 y4

@textWave2
av adj1 val 12500
av adj2 val 0
gd a1 pin 0 adj1 20000
gd a2 pin -10000 adj2 10000
gd y1 */ h a1 100000
gd dy2 */ y1 10 3
gd y2 +- y1 0 dy2
gd y3 +- y1 dy2 0
gd y4 +- b 0 y1
gd y5 +- y4 0 dy2
gd y6 +- y4 dy2 0
gd of */ w a2 100000
gd of2 */ w a2 50000
gd x1 abs of
gd dx2 ?: of2 0 of2
gd x2 +- l 0 dx2
gd dx5 ?: of2 of2 0
gd x5 +- r 0 dx5
gd dx3 +/ dx2 x5 3
gd x3 +- x2 dx3 0
gd x4 +/ x3 x5 2
gd x6 +- l dx5 0
gd x10 +- r dx2 0
gd x7 +- x6 dx3 0
gd x8 +/ x7 x10 2
gd x9 +- r 0 x1
gd xAdj +- hc of 0
path
M x2 y1; C x3 y3 x4 y2 x5 y1
path
M x6 y4; C x7 y6 x8 y5 x10 y4

@textWave4
av adj1 val 6250
av adj2 val 0
gd a1 pin 0 adj1 12500
gd a2 pin -10000 adj2 10000
gd y1 */ h a1 100000
gd dy2 */ y1 10 3
gd y2 +- y1 0 dy2
gd y3 +- y1 dy2 0
gd y4 +- b 0 y1
gd y5 +- y4 0 dy2
gd y6 +- y4 dy2 0
gd of */ w a2 100000
gd of2 */ w a2 50000
gd x1 abs of
gd dx2 ?: of2 0 of2
gd x2 +- l 0 dx2
gd dx8 ?: of2 of2 0
gd x8 +- r 0 dx8
gd dx3 +/ dx2 x8 6
gd x3 +- x2 dx3 0
gd dx4 +/ dx2 x8 3
gd x4 +- x2 dx4 0
gd x5 +/ x2 x8 2
gd x6 +- x5 dx3 0
gd x7 +/ x6 x8 2
gd x9 +- l dx8 0
gd x15 +- r dx2 0
gd x10 +- x9 dx3 0
gd x11 +- x9 dx4 0
gd x12 +/ x9 x15 2
gd x13 +- x12 dx3 0
gd x14 +/ x13 x15 2
gd x16 +- r 0 x1
gd xAdj +- hc of 0
path
M x2 y1; C x3 y3 x4 y2 x5 y1; C x6 y3 x7 y2 x8 y1
path
M x9 y4; C x10 y6 x11 y5 x12 y4; C x13 y6 x14 y5 x15 y4
