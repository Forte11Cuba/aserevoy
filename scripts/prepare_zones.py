#!/usr/bin/env python3
"""Extract the municipalities of a Cuban province from the OpenStreetMap
extract and save their simplified outlines as GeoJSON.

The bot uses them to show an approximate zone in the offers channel (which
municipality each pin falls in), never the exact location.

Usage:
    pip install osmium shapely
    python scripts/prepare_zones.py osrm-data/cuba-latest.osm.pbf data/zones.geojson

The extract comes from Geofabrik:
https://download.geofabrik.de/central-america/cuba-latest.osm.pbf
"""

import argparse
import json
import sys

import osmium
import shapely
from shapely import wkb

PROVINCE_LEVEL = "4"
MUNICIPALITY_LEVEL = "6"


class Boundaries(osmium.SimpleHandler):
    """Collects administrative areas of the province and municipality levels."""

    def __init__(self):
        super().__init__()
        self.factory = osmium.geom.WKBFactory()
        self.provinces = []
        self.municipalities = []

    def area(self, a):
        tags = a.tags
        if tags.get("boundary") != "administrative" or "name" not in tags:
            return
        level = tags.get("admin_level")
        if level not in (PROVINCE_LEVEL, MUNICIPALITY_LEVEL):
            return
        try:
            shape = wkb.loads(self.factory.create_multipolygon(a), hex=True)
        except RuntimeError as e:
            # Broken relations in OSM: skip them and say so.
            print(f"skipping {tags['name']}: {e}", file=sys.stderr)
            return
        target = self.provinces if level == PROVINCE_LEVEL else self.municipalities
        target.append((tags["name"], shape))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pbf", help="OSM extract of Cuba (.osm.pbf)")
    parser.add_argument("output", help="GeoJSON to write")
    parser.add_argument("--province", default="La Habana", help="province name in OSM (default: La Habana)")
    parser.add_argument(
        "--tolerance",
        type=float,
        default=0.0005,
        help="simplification tolerance in degrees (default 0.0005, about 50 m)",
    )
    args = parser.parse_args()

    boundaries = Boundaries()
    boundaries.apply_file(args.pbf, locations=True, idx="flex_mem")

    provinces = [shape for name, shape in boundaries.provinces if name == args.province]
    if not provinces:
        names = ", ".join(sorted(name for name, _ in boundaries.provinces))
        sys.exit(f"province {args.province!r} not found; provinces in the extract: {names}")
    province = shapely.union_all(provinces)

    features = []
    for name, shape in sorted(boundaries.municipalities, key=lambda m: m[0]):
        if not shape.representative_point().within(province):
            continue
        simple = shapely.set_precision(shape.simplify(args.tolerance, preserve_topology=True), 0.00001)
        features.append(
            {
                "type": "Feature",
                "properties": {"province": args.province, "municipality": name},
                "geometry": shapely.geometry.mapping(simple),
            }
        )
    if not features:
        sys.exit(f"no municipalities found inside {args.province!r}")

    with open(args.output, "w", encoding="utf-8") as f:
        json.dump({"type": "FeatureCollection", "features": features}, f, ensure_ascii=False, separators=(",", ":"))
        f.write("\n")
    print(f"{len(features)} municipalities of {args.province} written to {args.output}")
    for feature in features:
        print(f"  {feature['properties']['municipality']}")


if __name__ == "__main__":
    main()
