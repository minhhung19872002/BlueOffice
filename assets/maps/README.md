# Boundary data of map charts (`assets/maps`)

Used by the region map chart ("Biểu đồ bản đồ", chartex `regionMap`, CORE-CHART-006) of BlueSheet,
and of charts embedded in slides — `crates/bo-sheet/src/geo/`.

## `world/countries.bin` — countries of the world (bundled)

- **Source:** Natural Earth, **1:50m Cultural Vectors — Admin 0 – Countries**, version **5.1.1**
  (`ne_50m_admin_0_countries.VERSION.txt`), official download
  <https://naciscdn.org/naturalearth/50m/cultural/ne_50m_admin_0_countries.zip>
  (SHA-256 of the zip `5fed433373581fa648920435f937d95f2d3c0200e067409c6478dcdf1b853139`).
- **Licence / terms:** public domain. Natural Earth's terms of use
  (<https://www.naturalearthdata.com/about/terms-of-use/>, checked 2026-11-11): "All versions of
  Natural Earth raster + vector map data found on this website are in the public domain." … "No
  permission is needed to use Natural Earth. Crediting the authors is unnecessary." We credit it
  anyway: "Made with Natural Earth."
- **Note on borders:** Natural Earth draws de facto boundaries; it is a general-purpose world base
  map, not an official source for any country's borders. Maps of Viet Nam's provinces need the
  approved dataset below.

### How it was derived

`countries.bin` is generated, not edited by hand:

```bash
node scripts/build-world-map.mjs <path>/ne_50m_admin_0_countries.zip   # → assets/maps/world/countries.bin
```

The script (its header documents the binary format) reads the shapefile and its UTF-8 DBF from the
zip, keeps for each of the 242 features the codes `ISO_A2_EH` / `ISO_A3_EH` and the names `NAME_EN`,
`NAME_VI`, `NAME`, `NAME_LONG`, `ADMIN`, `FORMAL_EN`, simplifies every ring (Douglas–Peucker,
0.04°), quantises to 0.01°, drops rings under 0.02 square degrees (each country keeps its largest
ring) and delta-encodes the points. Result: 242 countries, 32,932 points, 89 KB.

## `vn-provinces/` — Vietnamese provinces (NOT bundled)

Province-level maps of Viet Nam (34 provinces and cities after the 2025 reorganisation) need a
boundary dataset whose licence a human has approved (see `docs/parity/NEEDS_HUMAN.md`). No such data
ships with BlueOffice; until it does, province maps show: "Bản đồ cấp tỉnh cần bộ dữ liệu ranh giới
tỉnh/thành được phê duyệt (assets/maps/vn-provinces)."

Dropping an approved dataset into this folder enables them without code changes — see
[`vn-provinces/README.md`](vn-provinces/README.md) for the expected files.
