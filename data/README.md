# Data files

## `streets.sqlite`

Named streets, the corners where two of them cross, neighborhoods (repartos)
and named places of La Habana, each with its municipality. The bot uses it to
read written addresses ("23 e/ L y M") and to show the nearest corner of a pin
in the offers channel. Generated with `scripts/prepare_osm.py` from the same
extract, after `zones.geojson`.

## `zones.geojson`

Simplified outlines of the municipalities of La Habana, used to show an
approximate zone in the offers channel. Generated with
`scripts/prepare_zones.py` from the Cuba extract by Geofabrik
(`cuba-latest.osm.pbf`).

Map data © [OpenStreetMap contributors](https://www.openstreetmap.org/copyright),
available under the [Open Database License (ODbL) 1.0](https://opendatacommons.org/licenses/odbl/1-0/).

Both files are derived databases and stay under the ODbL, not under the MIT
license that covers the project's code (see `LICENSE`). If you share a
modified version of them, they must keep this attribution and the same license.
