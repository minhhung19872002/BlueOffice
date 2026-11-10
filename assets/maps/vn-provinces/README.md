# Vietnamese province boundaries — dataset folder (empty on purpose)

No data ships here. Province maps need a boundary dataset of the current administrative units
(34 provinces and cities after the 2025 reorganisation) whose licence has been **approved by a
human** (`docs/parity/NEEDS_HUMAN.md`). Never hand-draw boundaries or add data with an unclear licence.

When an approved dataset exists, put these files in this folder — no code change is needed
(`crates/bo-sheet/src/geo/dataset.rs`; web: `packages/sheet-editor/src/mapDatasets.ts` fetches
`maps/vn-provinces/dataset.json` copied by `scripts/sync-assets.mjs`; CLI: `BLUEOFFICE_MAPS_DIR=assets/maps`):

1. `dataset.json`:

   ```json
   {
     "format": "bo-map-dataset/1",
     "id": "vn-provinces",
     "level": "state",
     "country": "VN",
     "file": "provinces.geojson",
     "nameProperties": ["ten", "name_en"],
     "codeProperty": "ma",
     "aliases": "aliases.json",
     "expectedCount": 34,
     "source": "<publisher, URL>",
     "version": "<date / edition>",
     "license": "<licence id and URL>"
   }
   ```

   `file` is GeoJSON (a `FeatureCollection` of `Polygon` / `MultiPolygon`) or TopoJSON (a
   `Topology`; `object` names the object to read). `nameProperties` are the feature properties
   holding names (the first is shown); names match with or without diacritics, case and the words
   "Tỉnh" / "Thành phố" / "TP.". `expectedCount` makes a file with a different number of units fail
   to load.

2. `aliases.json` (optional): former units and other spellings → a current name or code, e.g.
   `{ "<tên cũ>": "<tên hoặc mã mới>" }` (taken from the approved dataset's own documentation), so
   data typed with old names still finds the new province.

3. Record the source, version and licence in this README and in `THIRD_PARTY_NOTICES.md`.
