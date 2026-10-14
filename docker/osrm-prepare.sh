#!/bin/sh
# Prepares the OSRM routing data once: extract, partition and customize the
# Cuba extract (multi-level Dijkstra). Skips everything if already done, so
# it is cheap on every `docker compose up`. To update the map, delete the
# `osrm-data` volume (see docs/docker.md).
set -eu
cd /data
if [ -f prepared.ok ]; then
    echo "OSRM data already prepared"
    exit 0
fi
if [ ! -f cuba-latest.osm.pbf ]; then
    echo "cuba-latest.osm.pbf is missing (the download step should have fetched it)" >&2
    exit 1
fi
osrm-extract -p /opt/car.lua cuba-latest.osm.pbf
osrm-partition cuba-latest.osrm
osrm-customize cuba-latest.osrm
touch prepared.ok
echo "OSRM data ready"
