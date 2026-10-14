#!/usr/bin/env python3
"""Build data/streets.sqlite from the OpenStreetMap extract: named streets,
the corners where two of them cross, neighborhoods (repartos) and named
places of La Habana, each with its municipality.

The bot uses it to turn written addresses ("23 e/ L y M, Vedado") into
coordinates, and to show the nearest corner of a pin in the offers channel.
Names are stored as they are in OSM; the bot normalizes them.

Usage:
    pip install osmium shapely
    python scripts/prepare_osm.py osrm-data/cuba-latest.osm.pbf data/zones.geojson data/streets.sqlite

Run prepare_zones.py first: the municipalities come from data/zones.geojson.
"""

import argparse
import json
import os
import sqlite3
import sys
from collections import defaultdict

import osmium
from shapely.geometry import Point, shape
from shapely.strtree import STRtree

# Streets people give as addresses (no footways, tracks or motorway links).
STREET_TYPES = {
    "primary", "secondary", "tertiary", "residential", "unclassified",
    "living_street", "pedestrian", "trunk", "service",
}
PLACE_TYPES = {"suburb", "neighbourhood", "quarter", "village", "hamlet"}
# Named things people use as landmarks ("frente a Coppelia").
POI_KEYS = ("amenity", "shop", "tourism", "leisure", "historic", "healthcare")


def place_kind(tags):
    """'suburb' for a big reparto (Vedado, Miramar, Alamar), 'neighborhood' for
    a smaller one, the POI type for a landmark, else None."""
    if tags.get("place") == "suburb":
        return "suburb"
    if tags.get("place") in PLACE_TYPES:
        return "neighborhood"
    return next((tags.get(k) for k in POI_KEYS if tags.get(k)), None)


class Municipalities:
    def __init__(self, geojson_path):
        with open(geojson_path, encoding="utf-8") as f:
            features = json.load(f)["features"]
        self.names = [f["properties"]["municipality"] for f in features]
        self.shapes = [shape(f["geometry"]) for f in features]
        self.tree = STRtree(self.shapes)

    def of(self, lon, lat):
        point = Point(lon, lat)
        for i in self.tree.query(point):
            if self.shapes[i].contains(point):
                return self.names[i]
        return None


class Collector(osmium.SimpleHandler):
    def __init__(self, municipalities):
        super().__init__()
        self.mun = municipalities
        # node id -> (lon, lat, {street names}) for nodes of named streets
        self.street_nodes = defaultdict(set)
        self.node_loc = {}
        self.streets = []  # (name, lon, lat, municipality)
        self.places = []  # (name, kind, lon, lat, municipality)

    def way(self, w):
        name = w.tags.get("name")
        if not name:
            return
        if w.tags.get("highway") not in STREET_TYPES:
            # Many landmarks (hospitals, hotels, parks) are drawn as areas.
            kind = place_kind(w.tags)
            coords = [(n.lon, n.lat) for n in w.nodes if n.location.valid()]
            if kind and coords:
                lon = sum(c[0] for c in coords) / len(coords)
                lat = sum(c[1] for c in coords) / len(coords)
                municipality = self.mun.of(lon, lat)
                if municipality:
                    self.places.append((name, kind, lon, lat, municipality))
            return
        coords = []
        for n in w.nodes:
            if not n.location.valid():
                continue
            coords.append((n.lon, n.lat))
            self.street_nodes[n.ref].add(name)
            self.node_loc[n.ref] = (n.lon, n.lat)
        if not coords:
            return
        # The way's middle node stands for it (good enough for short city blocks).
        lon, lat = coords[len(coords) // 2]
        municipality = self.mun.of(lon, lat)
        if municipality:
            self.streets.append((name, lon, lat, municipality))

    def node(self, n):
        name = n.tags.get("name")
        kind = place_kind(n.tags)
        if not name or not kind:
            return
        municipality = self.mun.of(n.location.lon, n.location.lat)
        if municipality:
            self.places.append((name, kind, n.location.lon, n.location.lat, municipality))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pbf", help="OSM extract of Cuba (.osm.pbf)")
    parser.add_argument("zones", help="data/zones.geojson (municipalities)")
    parser.add_argument("output", help="SQLite file to write")
    args = parser.parse_args()

    mun = Municipalities(args.zones)
    c = Collector(mun)
    c.apply_file(args.pbf, locations=True)

    corners = []
    for node, names in c.street_nodes.items():
        if len(names) < 2:
            continue
        lon, lat = c.node_loc[node]
        municipality = mun.of(lon, lat)
        if not municipality:
            continue
        names = sorted(names)
        for i, a in enumerate(names):
            for b in names[i + 1:]:
                corners.append((a, b, lon, lat, municipality))

    if os.path.exists(args.output):
        os.remove(args.output)
    db = sqlite3.connect(args.output)
    db.executescript(
        """
        CREATE TABLE streets (name TEXT NOT NULL, lon REAL NOT NULL, lat REAL NOT NULL, municipality TEXT NOT NULL);
        CREATE TABLE corners (street_a TEXT NOT NULL, street_b TEXT NOT NULL, lon REAL NOT NULL, lat REAL NOT NULL,
                              municipality TEXT NOT NULL);
        CREATE TABLE places (name TEXT NOT NULL, kind TEXT NOT NULL, lon REAL NOT NULL, lat REAL NOT NULL,
                             municipality TEXT NOT NULL);
        """
    )
    db.executemany("INSERT INTO streets VALUES (?, ?, ?, ?)", c.streets)
    db.executemany("INSERT INTO corners VALUES (?, ?, ?, ?, ?)", corners)
    db.executemany("INSERT INTO places VALUES (?, ?, ?, ?, ?)", c.places)
    db.commit()
    db.execute("VACUUM")
    db.close()

    if not corners:
        sys.exit("no corners found: check the extract and the zones")
    print(f"{len(c.streets)} street segments, {len(corners)} corners, {len(c.places)} places -> {args.output}")


if __name__ == "__main__":
    main()
