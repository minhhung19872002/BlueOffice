//! Preset shape definitions — ECMA-376 Part 1, §20.1.9.18 prstGeom and the
//! presetShapeDefinitions annex (§20.1.10.56 ST_ShapeType).
//!
//! Each definition is transcribed by hand into a compact line format that maps
//! 1:1 onto the annex's XML (`avLst`, `gdLst`, `rect`, `pathLst`):
//!
//! ```text
//! @name                      start of a shape
//! av <name> <fmla>           adjust value (default)
//! gd <name> <fmla>           guide
//! rect <l> <t> <r> <b>       text rectangle
//! path [w=] [h=] [fill=] [stroke=0] [ext=0]
//! M x y; L x y; A wR hR stAng swAng; Q x1 y1 x y; C x1 y1 x2 y2 x y; Z
//! ```

use bo_model::slide::{CustomGeometry, GeomPath, Guide, PathCommand};
use std::collections::HashMap;
use std::sync::OnceLock;

const DEFS: &str = r"
@rect
path
M l t; L r t; L r b; L l b; Z

@roundRect
av adj val 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
gd il */ x1 29289 100000
gd ir +- r 0 il
gd ib +- b 0 il
rect il il ir ib
path
M l x1; A x1 x1 cd2 cd4; L x2 t; A x1 x1 3cd4 cd4; L r y2; A x1 x1 0 cd4; L x1 b; A x1 x1 cd4 cd4; Z

@ellipse
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect il it ir ib
path
M l vc; A wd2 hd2 cd2 cd4; A wd2 hd2 3cd4 cd4; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z

@flowChartConnector
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect il it ir ib
path
M l vc; A wd2 hd2 cd2 cd4; A wd2 hd2 3cd4 cd4; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z

@triangle
av adj val 50000
gd a pin 0 adj 100000
gd x1 */ w a 200000
gd x2 */ w a 100000
gd x3 +- x1 wd2 0
rect x1 vc x3 b
path
M l b; L x2 t; L r b; Z

@rtTriangle
gd it */ h 7 12
gd ir */ w 7 12
gd ib */ h 11 12
rect l it ir ib
path
M l b; L l t; L r b; Z

@diamond
gd ir */ w 3 4
gd ib */ h 3 4
rect wd4 hd4 ir ib
path
M l vc; L hc t; L r vc; L hc b; Z

@parallelogram
av adj val 25000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 200000
gd x2 */ ss a 100000
gd x6 +- r 0 x2
gd x5 +- r 0 x1
path
M l b; L x2 t; L r t; L x6 b; Z

@trapezoid
av adj val 25000
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 200000
gd x2 */ ss a 100000
gd x3 +- r 0 x2
gd x4 +- r 0 x1
path
M l b; L x2 t; L x3 t; L r b; Z

@pentagon
av hf val 105146
av vf val 110557
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc vf 100000
gd dx1 cos swd2 1080000
gd dx2 cos swd2 18360000
gd dy1 sin shd2 1080000
gd dy2 sin shd2 18360000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- svc 0 dy1
gd y2 +- svc 0 dy2
path
M x1 y1; L hc t; L x4 y1; L x3 y2; L x2 y2; Z

@hexagon
av adj val 25000
av vf val 115470
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd shd2 */ hd2 vf 100000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd dy1 sin shd2 3600000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
path
M l vc; L x1 y1; L x2 y1; L r vc; L x2 y2; L x1 y2; Z

@octagon
av adj val 29289
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
path
M l x1; L x1 t; L x2 t; L r x1; L r y2; L x2 b; L x1 b; L l y2; Z

@star4
av adj val 12500
gd a pin 0 adj 50000
gd iwd2 */ wd2 a 50000
gd ihd2 */ hd2 a 50000
gd sdx cos iwd2 2700000
gd sdy sin ihd2 2700000
gd sx1 +- hc 0 sdx
gd sx2 +- hc sdx 0
gd sy1 +- vc 0 sdy
gd sy2 +- vc sdy 0
rect sx1 sy1 sx2 sy2
path
M l vc; L sx1 sy1; L hc t; L sx2 sy1; L r vc; L sx2 sy2; L hc b; L sx1 sy2; Z

@star5
av adj val 19098
av hf val 105146
av vf val 110557
gd a pin 0 adj 50000
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc vf 100000
gd dx1 cos swd2 1080000
gd dx2 cos swd2 18360000
gd dy1 sin shd2 1080000
gd dy2 sin shd2 18360000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- svc 0 dy1
gd y2 +- svc 0 dy2
gd iwd2 */ swd2 a 50000
gd ihd2 */ shd2 a 50000
gd sdx1 cos iwd2 20520000
gd sdx2 cos iwd2 3240000
gd sdy1 sin ihd2 3240000
gd sdy2 sin ihd2 20520000
gd sx1 +- hc 0 sdx1
gd sx2 +- hc 0 sdx2
gd sx3 +- hc sdx2 0
gd sx4 +- hc sdx1 0
gd sy1 +- svc 0 sdy1
gd sy2 +- svc 0 sdy2
gd sy3 +- svc ihd2 0
rect sx1 sy1 sx4 sy3
path
M x1 y1; L sx2 sy1; L hc t; L sx3 sy1; L x4 y1; L sx4 sy2; L x3 y2; L hc sy3; L x2 y2; L sx1 sy2; Z

@star6
av adj val 28868
av hf val 115470
gd a pin 0 adj 50000
gd swd2 */ wd2 hf 100000
gd dx1 cos swd2 1800000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
gd y2 +- vc hd4 0
gd iwd2 */ swd2 a 50000
gd ihd2 */ hd2 a 50000
gd sdx2 */ iwd2 1 2
gd sx1 +- hc 0 iwd2
gd sx2 +- hc 0 sdx2
gd sx3 +- hc sdx2 0
gd sx4 +- hc iwd2 0
gd sdy1 sin ihd2 3600000
gd sy1 +- vc 0 sdy1
gd sy2 +- vc sdy1 0
rect sx1 sy1 sx4 sy2
path
M x1 hd4; L sx2 sy1; L hc t; L sx3 sy1; L x2 hd4; L sx4 vc; L x2 y2; L sx3 sy2; L hc b; L sx2 sy2; L x1 y2; L sx1 vc; Z

@plus
av adj val 25000
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
gd d +- w 0 h
gd il ?: d l x1
gd ir ?: d r x2
gd it ?: d x1 t
gd ib ?: d y2 b
rect il it ir ib
path
M l x1; L x1 x1; L x1 t; L x2 t; L x2 x1; L r x1; L r y2; L x2 y2; L x2 b; L x1 b; L x1 y2; L l y2; Z

@rightArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx1 */ ss a2 100000
gd x1 +- r 0 dx1
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd dx2 */ y1 dx1 hd2
gd x2 +- x1 dx2 0
rect l y1 x2 y2
path
M l y1; L x1 y1; L x1 t; L r vc; L x1 b; L x1 y2; L l y2; Z

@leftArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx2 */ ss a2 100000
gd x2 +- l dx2 0
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd dx1 */ y1 dx2 hd2
gd x1 +- x2 0 dx1
rect x1 y1 r y2
path
M l vc; L x2 t; L x2 y1; L r y1; L r y2; L x2 y2; L x2 b; Z

@upArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dy2 */ ss a2 100000
gd y2 +- t dy2 0
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
gd dy1 */ x1 dy2 wd2
gd y1 +- y2 0 dy1
rect x1 y1 x2 b
path
M l y2; L hc t; L r y2; L x2 y2; L x2 b; L x1 b; L x1 y2; Z

@downArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dy1 */ ss a2 100000
gd y1 +- b 0 dy1
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
gd dy2 */ x1 dy1 wd2
gd y2 +- y1 dy2 0
rect x1 t x2 y2
path
M l y1; L x1 y1; L x1 t; L x2 t; L x2 y1; L r y1; L hc b; Z

@leftRightArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 50000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd x2 */ ss a2 100000
gd x3 +- r 0 x2
gd dy */ h a1 200000
gd y1 +- vc 0 dy
gd y2 +- vc dy 0
path
M l vc; L x2 t; L x2 y1; L x3 y1; L x3 t; L r vc; L x3 b; L x3 y2; L x2 y2; L x2 b; Z

@upDownArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 50000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd y2 */ ss a2 100000
gd y3 +- b 0 y2
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
path
M l y2; L hc t; L r y2; L x2 y2; L x2 y3; L r y3; L hc b; L l y3; L x1 y3; L x1 y2; Z

@notchedRightArrow
av adj1 val 50000
av adj2 val 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx2 */ ss a2 100000
gd x2 +- r 0 dx2
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd x1 */ dy1 dx2 hd2
path
M l y1; L x2 y1; L x2 t; L r vc; L x2 b; L x2 y2; L l y2; L x1 vc; Z

@chevron
av adj val 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 100000
gd x2 +- r 0 x1
path
M l t; L x2 t; L r vc; L x2 b; L l b; L x1 vc; Z

@homePlate
av adj val 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
path
M l t; L x1 t; L r vc; L x1 b; L l b; Z

@can
av adj val 25000
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 200000
gd y2 +- y1 y1 0
gd y3 +- b 0 y1
rect l y2 r y3
path stroke=0 ext=0
M l y1; A wd2 y1 cd2 -10800000; L r y3; A wd2 y1 0 cd2; Z
path fill=lighten stroke=0 ext=0
M l y1; A wd2 y1 cd2 cd2; A wd2 y1 0 cd2; Z
path fill=none
M r y1; A wd2 y1 0 cd2; A wd2 y1 cd2 cd2; L r y3; A wd2 y1 0 cd2; L l y1

@cube
av adj val 25000
gd a pin 0 adj 100000
gd y1 */ ss a 100000
gd y4 +- b 0 y1
gd y2 */ y4 1 2
gd y3 +/ y1 b 2
gd x4 +- r 0 y1
gd x2 */ x4 1 2
gd x3 +/ y1 r 2
rect l y1 x4 b
path stroke=0 ext=0
M l y1; L x4 y1; L x4 b; L l b; Z
path fill=darkenLess stroke=0 ext=0
M x4 y1; L r t; L r y4; L x4 b; Z
path fill=lightenLess stroke=0 ext=0
M l y1; L y1 t; L r t; L x4 y1; Z
path fill=none ext=0
M l y1; L y1 t; L r t; L r y4; L x4 b; L l b; Z; M l y1; L x4 y1; L r t; M x4 y1; L x4 b

@donut
av adj val 25000
gd a pin 0 adj 50000
gd dr */ ss a 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
path
M l vc; A wd2 hd2 cd2 cd4; A wd2 hd2 3cd4 cd4; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z; M dr vc; A iwd2 ihd2 cd2 -5400000; A iwd2 ihd2 cd4 -5400000; A iwd2 ihd2 0 -5400000; A iwd2 ihd2 3cd4 -5400000; Z

@noSmoking
av adj val 18750
gd a pin 0 adj 50000
gd dr */ ss a 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
gd ang at2 w h
gd ct cos ihd2 ang
gd st sin iwd2 ang
gd m mod ct st 0
gd n */ iwd2 ihd2 m
gd drd2 */ dr 1 2
gd dang at2 n drd2
gd dang2 */ dang 2 1
gd swAng +- -10800000 dang2 0
gd t3 at2 w h
gd stAng1 +- t3 0 dang
gd stAng2 +- stAng1 0 cd2
gd ct1 cos ihd2 stAng1
gd st1 sin iwd2 stAng1
gd m1 mod ct1 st1 0
gd n1 */ iwd2 ihd2 m1
gd dx1 cos n1 stAng1
gd dy1 sin n1 stAng1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd x2 +- hc 0 dx1
gd y2 +- vc 0 dy1
path
M l vc; A wd2 hd2 cd2 cd4; A wd2 hd2 3cd4 cd4; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z; M x1 y1; A iwd2 ihd2 stAng1 swAng; Z; M x2 y2; A iwd2 ihd2 stAng2 swAng; Z

@blockArc
av adj1 val 10800000
av adj2 val 0
av adj3 val 25000
gd stAng pin 0 adj1 21599999
gd istAng pin 0 adj2 21599999
gd a3 pin 0 adj3 50000
gd sw11 +- istAng 0 stAng
gd sw12 +- sw11 21600000 0
gd swAng ?: sw11 sw11 sw12
gd iswAng +- 0 0 swAng
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd dr */ ss a3 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
gd wt2 sin iwd2 istAng
gd ht2 cos ihd2 istAng
gd dx2 cat2 iwd2 ht2 wt2
gd dy2 sat2 ihd2 ht2 wt2
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
path
M x1 y1; A wd2 hd2 stAng swAng; L x2 y2; A iwd2 ihd2 istAng iswAng; Z

@pie
av adj1 val 0
av adj2 val 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path
M x1 y1; A wd2 hd2 stAng swAng; L hc vc; Z

@arc
av adj1 val 16200000
av adj2 val 0
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw11 +- enAng 0 stAng
gd sw12 +- sw11 21600000 0
gd swAng ?: sw11 sw11 sw12
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path stroke=0 ext=0
M x1 y1; A wd2 hd2 stAng swAng; L hc vc; Z
path fill=none
M x1 y1; A wd2 hd2 stAng swAng

@chord
av adj1 val 2700000
av adj2 val 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path
M x1 y1; A wd2 hd2 stAng swAng; Z

@heart
gd dx1 */ w 49 48
gd dx2 */ w 10 48
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- t 0 hd3
gd il */ w 1 6
gd ir */ w 5 6
gd ib */ h 2 3
rect il hd4 ir ib
path
M hc hd4; C x3 y1 x4 hd4 hc b; C x1 hd4 x2 y1 hc hd4; Z

@line
path
M l t; L r b

@straightConnector1
path fill=none
M l t; L r b

@bentConnector2
path fill=none
M l t; L r t; L r b

@bentConnector3
av adj1 val 50000
gd x1 */ w adj1 100000
path fill=none
M l t; L x1 t; L x1 b; L r b

@curvedConnector2
path fill=none
M l t; C wd2 t r hd2 r b

@curvedConnector3
av adj1 val 50000
gd x2 */ w adj1 100000
gd x1 +/ l x2 2
gd x3 +/ r x2 2
gd y3 */ h 3 4
path fill=none
M l t; C x1 t x2 hd4 x2 vc; C x2 y3 x3 b r b

@flowChartProcess
path w=1 h=1
M 0 0; L 1 0; L 1 1; L 0 1; Z

@flowChartDecision
gd ir */ w 3 4
gd ib */ h 3 4
rect wd4 hd4 ir ib
path w=2 h=2
M 0 1; L 1 0; L 2 1; L 1 2; Z

@flowChartTerminator
gd il */ w 1018 21600
gd ir */ w 20582 21600
gd it */ h 3163 21600
gd ib */ h 18437 21600
rect il it ir ib
path w=21600 h=21600
M 3475 0; L 18125 0; A 3475 10800 3cd4 cd2; L 3475 21600; A 3475 10800 cd4 cd2; Z

@flowChartAlternateProcess
gd x2 +- r 0 ssd6
gd y2 +- b 0 ssd6
gd il */ ssd6 29289 100000
gd ir +- r 0 il
gd ib +- b 0 il
rect il il ir ib
path
M l ssd6; A ssd6 ssd6 cd2 cd4; L x2 t; A ssd6 ssd6 3cd4 cd4; L r y2; A ssd6 ssd6 0 cd4; L ssd6 b; A ssd6 ssd6 cd4 cd4; Z

@flowChartInputOutput
gd x3 */ w 2 5
gd x4 */ w 3 5
gd x5 */ w 4 5
gd x6 */ w 9 10
rect wd5 t x5 b
path w=5 h=5
M 0 5; L 1 0; L 5 0; L 4 5; Z

@flowChartDocument
gd y1 */ h 17322 21600
gd y2 */ h 20172 21600
rect l t r y1
path w=21600 h=21600
M 0 0; L 21600 0; L 21600 17322; C 10800 17322 10800 23922 0 20172; Z

@flowChartManualInput
rect l hd5 r b
path w=5 h=5
M 0 1; L 5 0; L 5 5; L 0 5; Z

@flowChartPredefinedProcess
gd x2 */ w 7 8
rect wd8 t x2 b
path stroke=0 w=1 h=1
M 0 0; L 1 0; L 1 1; L 0 1; Z
path fill=none w=8 h=8
M 1 0; L 1 8; M 7 0; L 7 8
path fill=none w=1 h=1
M 0 0; L 1 0; L 1 1; L 0 1; Z

@flowChartPreparation
gd x2 */ w 4 5
rect wd5 t x2 b
path w=10 h=10
M 0 5; L 2 0; L 8 0; L 10 5; L 8 10; L 2 10; Z

@flowChartManualOperation
gd x3 */ w 4 5
gd x4 */ w 9 10
rect wd5 t x3 b
path w=5 h=5
M 0 0; L 5 0; L 4 5; L 1 5; Z

@flowChartOffpageConnector
gd y1 */ h 4 5
rect l t r y1
path w=10 h=10
M 0 0; L 10 0; L 10 8; L 5 10; L 0 8; Z

@flowChartPunchedCard
rect l hd5 r b
path w=5 h=5
M 0 1; L 1 0; L 5 0; L 5 5; L 0 5; Z

@flowChartMerge
gd x2 */ w 3 4
rect wd4 t x2 vc
path w=2 h=2
M 0 0; L 2 0; L 1 2; Z

@flowChartExtract
gd x2 */ w 3 4
rect wd4 vc x2 b
path w=2 h=2
M 0 2; L 1 0; L 2 2; Z

@flowChartCollate
gd ir */ w 3 4
gd ib */ h 3 4
rect wd4 hd4 ir ib
path w=2 h=2
M 0 0; L 2 0; L 1 1; L 2 2; L 0 2; L 1 1; Z

@flowChartSort
gd ir */ w 3 4
gd ib */ h 3 4
rect wd4 hd4 ir ib
path stroke=0 w=2 h=2
M 0 1; L 1 0; L 2 1; L 1 2; Z
path fill=none w=2 h=2
M 0 1; L 2 1
path fill=none w=2 h=2
M 0 1; L 1 0; L 2 1; L 1 2; Z

@flowChartDelay
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
rect l it ir ib
path
M l t; L hc t; A wd2 hd2 3cd4 cd2; L l b; Z

@wedgeRectCallout
av adj1 val -20833
av adj2 val 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd dx */ dxPos h w
gd dy */ dyPos 1 1
gd adx abs dx
gd ady abs dy
gd dq +- adx 0 ady
gd xg1 ?: dxPos 7 2
gd xg2 ?: dxPos 10 5
gd x1 */ w xg1 12
gd x2 */ w xg2 12
gd yg1 ?: dyPos 7 2
gd yg2 ?: dyPos 10 5
gd y1 */ h yg1 12
gd y2 */ h yg2 12
gd t1 ?: dxPos l xPos
gd xl ?: dq t1 l
gd t2 ?: dyPos x1 xPos
gd xt ?: dq x1 t2
gd t3 ?: dxPos xPos r
gd xr ?: dq t3 r
gd t4 ?: dyPos xPos x1
gd xb ?: dq x1 t4
gd t5 ?: dxPos y1 yPos
gd yl ?: dq t5 y1
gd t6 ?: dyPos t yPos
gd yt ?: dq t t6
gd t7 ?: dxPos yPos y1
gd yr ?: dq t7 y1
gd t8 ?: dyPos yPos b
gd yb ?: dq b t8
path
M l t; L x1 t; L xt yt; L x2 t; L r t; L r y1; L xr yr; L r y2; L r b; L x2 b; L xb yb; L x1 b; L l b; L l y2; L xl yl; L l y1; Z

@wedgeRoundRectCallout
av adj1 val -20833
av adj2 val 62500
av adj3 val 16667
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd dq */ dxPos h w
gd ady abs dyPos
gd adq abs dq
gd dz +- adq 0 ady
gd xg1 ?: dxPos 7 2
gd xg2 ?: dxPos 10 5
gd x1 */ w xg1 12
gd x2 */ w xg2 12
gd yg1 ?: dyPos 7 2
gd yg2 ?: dyPos 10 5
gd y1 */ h yg1 12
gd y2 */ h yg2 12
gd t1 ?: dxPos l xPos
gd xl ?: dz t1 l
gd t2 ?: dyPos x1 xPos
gd xt ?: dz x1 t2
gd t3 ?: dxPos xPos r
gd xr ?: dz t3 r
gd t4 ?: dyPos xPos x1
gd xb ?: dz x1 t4
gd t5 ?: dxPos y1 yPos
gd yl ?: dz t5 y1
gd t6 ?: dyPos t yPos
gd yt ?: dz t t6
gd t7 ?: dxPos yPos y1
gd yr ?: dz t7 y1
gd t8 ?: dyPos yPos b
gd yb ?: dz b t8
gd u1 */ ss adj3 100000
gd u2 +- r 0 u1
gd v2 +- b 0 u1
gd il */ u1 29289 100000
gd ir +- r 0 il
gd ib +- b 0 il
rect il il ir ib
path
M l u1; A u1 u1 cd2 cd4; L x1 t; L xt yt; L x2 t; L u2 t; A u1 u1 3cd4 cd4; L r y1; L xr yr; L r y2; L r v2; A u1 u1 0 cd4; L x2 b; L xb yb; L x1 b; L u1 b; A u1 u1 cd4 cd4; L l y2; L xl yl; L l y1; Z

@snip1Rect
av adj val 16667
gd a pin 0 adj 50000
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
gd it */ dx1 1 2
gd ir +/ x1 r 2
rect l it ir b
path
M l t; L x1 t; L r dx1; L r b; L l b; Z

@snip2SameRect
av adj1 val 16667
av adj2 val 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
path
M tx1 t; L tx2 t; L r tx1; L r by1; L bx2 b; L bx1 b; L l by1; L l tx1; Z

@snip2DiagRect
av adj1 val 0
av adj2 val 16667
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd lx1 */ ss a1 100000
gd lx2 +- r 0 lx1
gd ly1 +- b 0 lx1
gd rx1 */ ss a2 100000
gd rx2 +- r 0 rx1
gd ry1 +- b 0 rx1
path
M lx1 t; L rx2 t; L r rx1; L r ly1; L lx2 b; L rx1 b; L l ry1; L l lx1; Z

@snipRoundRect
av adj1 val 16667
av adj2 val 16667
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd x1 */ ss a1 100000
gd dx2 */ ss a2 100000
gd x2 +- r 0 dx2
path
M x1 t; L x2 t; L r dx2; L r b; L l b; L l x1; A x1 x1 cd2 cd4; Z

@round1Rect
av adj val 16667
gd a pin 0 adj 50000
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
gd idx */ dx1 29289 100000
gd ir +- r 0 idx
rect l t ir b
path
M l t; L x1 t; A dx1 dx1 3cd4 cd4; L r b; L l b; Z

@round2SameRect
av adj1 val 16667
av adj2 val 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
path
M tx1 t; L tx2 t; A tx1 tx1 3cd4 cd4; L r by1; A bx1 bx1 0 cd4; L bx1 b; A bx1 bx1 cd4 cd4; L l tx1; A tx1 tx1 cd2 cd4; Z

@round2DiagRect
av adj1 val 16667
av adj2 val 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd x1 */ ss a1 100000
gd y1 +- b 0 x1
gd a */ ss a2 100000
gd x2 +- r 0 a
gd y2 +- b 0 a
path
M x1 t; L x2 t; A a a 3cd4 cd4; L r y1; A x1 x1 0 cd4; L a b; A a a cd4 cd4; L l x1; A x1 x1 cd2 cd4; Z

@frame
av adj1 val 12500
gd a1 pin 0 adj1 50000
gd x1 */ ss a1 100000
gd x4 +- r 0 x1
gd y4 +- b 0 x1
rect x1 x1 x4 y4
path
M l t; L r t; L r b; L l b; Z; M x1 x1; L x1 y4; L x4 y4; L x4 x1; Z

@halfFrame
av adj1 val 33333
av adj2 val 33333
gd maxAdj2 */ 100000 w ss
gd a2 pin 0 adj2 maxAdj2
gd x1 */ ss a2 100000
gd g1 */ h x1 w
gd g2 +- h 0 g1
gd maxAdj1 */ 100000 g2 ss
gd a1 pin 0 adj1 maxAdj1
gd y1 */ ss a1 100000
gd dx2 */ y1 w h
gd x2 +- r 0 dx2
gd dy2 */ x1 h w
gd y2 +- b 0 dy2
path
M l t; L r t; L x2 y1; L x1 y1; L x1 y2; L l b; Z

@corner
av adj1 val 50000
av adj2 val 50000
gd maxAdj1 */ 100000 h ss
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 maxAdj1
gd a2 pin 0 adj2 maxAdj2
gd x1 */ ss a2 100000
gd dy1 */ ss a1 100000
gd y1 +- b 0 dy1
path
M l t; L x1 t; L x1 y1; L r y1; L r b; L l b; Z

@plaque
av adj val 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
gd il */ x1 70711 100000
gd ir +- r 0 il
gd ib +- b 0 il
rect il il ir ib
path
M l x1; A x1 x1 cd4 -5400000; L x2 t; A x1 x1 cd2 -5400000; L r y2; A x1 x1 3cd4 -5400000; L x1 b; A x1 x1 0 -5400000; Z

@foldedCorner
av adj val 16667
gd a pin 0 adj 50000
gd dy2 */ ss a 100000
gd dy1 */ dy2 1 5
gd x1 +- r 0 dy2
gd x2 +- x1 dy1 0
gd y2 +- b 0 dy2
gd y1 +- y2 dy1 0
rect l t r y2
path stroke=0 ext=0
M l t; L r t; L r y2; L x1 b; L l b; Z
path fill=darkenLess stroke=0 ext=0
M x1 b; L x2 y1; L r y2; Z
path fill=none ext=0
M x1 b; L x2 y1; L r y2; L x1 b; L l b; L l t; L r t; L r y2

@bracketPair
av adj val 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
gd il */ x1 29289 100000
gd ir +- r 0 il
gd ib +- b 0 il
rect il il ir ib
path stroke=0 ext=0
M l x1; A x1 x1 cd2 cd4; L x2 t; A x1 x1 3cd4 cd4; L r y2; A x1 x1 0 cd4; L x1 b; A x1 x1 cd4 cd4; Z
path fill=none
M x1 b; A x1 x1 cd4 cd4; L l x1; A x1 x1 cd2 cd4; M x2 t; A x1 x1 3cd4 cd4; L r y2; A x1 x1 0 cd4

@bracePair
av adj val 8333
gd a pin 0 adj 25000
gd x1 */ ss a 100000
gd x2 */ ss a 50000
gd x3 +- r 0 x2
gd x4 +- r 0 x1
gd y2 +- vc 0 x1
gd y3 +- vc x1 0
gd y4 +- b 0 x1
gd it */ x1 29289 100000
gd il +- x1 it 0
gd ir +- r 0 il
gd ib +- b 0 it
rect il il ir ib
path stroke=0 ext=0
M x2 b; A x1 x1 cd4 cd4; L x1 y3; A x1 x1 0 -5400000; A x1 x1 cd4 -5400000; L x1 x1; A x1 x1 cd2 cd4; L x3 t; A x1 x1 3cd4 cd4; L x4 y2; A x1 x1 cd2 -5400000; A x1 x1 3cd4 -5400000; L x4 y4; A x1 x1 0 cd4; Z
path fill=none
M x2 b; A x1 x1 cd4 cd4; L x1 y3; A x1 x1 0 -5400000; A x1 x1 cd4 -5400000; L x1 x1; A x1 x1 cd2 cd4; M x3 t; A x1 x1 3cd4 cd4; L x4 y2; A x1 x1 cd2 -5400000; A x1 x1 3cd4 -5400000; L x4 y4; A x1 x1 0 cd4

@leftBracket
av adj val 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
gd dx1 cos w 2700000
gd dy1 sin y1 2700000
gd il +- r 0 dx1
gd it +- y1 0 dy1
gd ib +- b dy1 y1
rect il it r ib
path stroke=0 ext=0
M r b; A w y1 cd4 cd4; L l y1; A w y1 cd2 cd4; Z
path fill=none
M r b; A w y1 cd4 cd4; L l y1; A w y1 cd2 cd4

@rightBracket
av adj val 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
gd dx1 cos w 2700000
gd dy1 sin y1 2700000
gd y3 +- y1 0 dy1
gd ir +- l dx1 0
gd y4 +- b dy1 y1
rect l y3 ir y4
path stroke=0 ext=0
M l t; A w y1 3cd4 cd4; L r y2; A w y1 0 cd4; Z
path fill=none
M l t; A w y1 3cd4 cd4; L r y2; A w y1 0 cd4
";

fn parse_defs() -> HashMap<&'static str, CustomGeometry> {
    let mut out = HashMap::new();
    let mut name: Option<&'static str> = None;
    let mut cur = CustomGeometry::default();
    let s = |v: &str| v.to_owned();
    for line in DEFS.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(n) = line.strip_prefix('@') {
            if let Some(prev) = name.take() {
                out.insert(prev, std::mem::take(&mut cur));
            }
            name = Some(n);
            continue;
        }
        let mut words = line.split_whitespace();
        let head = words.next().unwrap_or_default();
        match head {
            "av" | "gd" => {
                let gname = words.next().unwrap_or_default().to_owned();
                let fmla = words.collect::<Vec<_>>().join(" ");
                let g = Guide { name: gname, fmla };
                if head == "av" {
                    cur.av.push(g);
                } else {
                    cur.gd.push(g);
                }
            }
            "rect" => {
                let v: Vec<&str> = words.collect();
                if v.len() == 4 {
                    cur.rect = Some([s(v[0]), s(v[1]), s(v[2]), s(v[3])]);
                }
            }
            "path" => {
                let mut p = GeomPath::default();
                for kv in words {
                    match kv.split_once('=') {
                        Some(("w", v)) => p.w = v.parse().ok(),
                        Some(("h", v)) => p.h = v.parse().ok(),
                        Some(("fill", v)) => p.fill = Some(v.to_owned()),
                        Some(("stroke", v)) => p.stroke = Some(v != "0"),
                        Some(("ext", v)) => p.extrusion_ok = Some(v != "0"),
                        _ => {}
                    }
                }
                cur.paths.push(p);
            }
            _ => {
                let Some(p) = cur.paths.last_mut() else {
                    continue;
                };
                for cmd in line.split(';').map(str::trim).filter(|c| !c.is_empty()) {
                    let a: Vec<&str> = cmd.split_whitespace().collect();
                    let c = match (a[0], a.len()) {
                        ("M", 3) => PathCommand::MoveTo(s(a[1]), s(a[2])),
                        ("L", 3) => PathCommand::LineTo(s(a[1]), s(a[2])),
                        ("A", 5) => PathCommand::ArcTo {
                            w_r: s(a[1]),
                            h_r: s(a[2]),
                            st_ang: s(a[3]),
                            sw_ang: s(a[4]),
                        },
                        ("Q", 5) => PathCommand::QuadTo([s(a[1]), s(a[2]), s(a[3]), s(a[4])]),
                        ("C", 7) => PathCommand::CubicTo([
                            s(a[1]),
                            s(a[2]),
                            s(a[3]),
                            s(a[4]),
                            s(a[5]),
                            s(a[6]),
                        ]),
                        ("Z", 1) => PathCommand::Close,
                        _ => continue,
                    };
                    p.commands.push(c);
                }
            }
        }
    }
    if let Some(prev) = name {
        out.insert(prev, cur);
    }
    out
}

fn table() -> &'static HashMap<&'static str, CustomGeometry> {
    static T: OnceLock<HashMap<&'static str, CustomGeometry>> = OnceLock::new();
    T.get_or_init(parse_defs)
}

/// Definition of a preset shape (`None` when not implemented).
pub fn preset(name: &str) -> Option<CustomGeometry> {
    table().get(name).cloned()
}

/// Names of all implemented presets, sorted.
pub fn preset_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = table().keys().copied().collect();
    v.sort_unstable();
    v
}
