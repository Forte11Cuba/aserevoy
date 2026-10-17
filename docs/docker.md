# Asere Voy con Docker

El bot y su propio OSRM (el motor que calcula los km de cada viaje para el precio) se arrancan juntos con Docker Compose. Es lo mismo en local y en el VPS.

## Arrancar

```bash
cp .env.example .env      # y rellénalo (token, canal, claves, superadmins…)
docker compose up -d --build
docker compose logs -f bot
```

El **primer arranque** descarga el mapa de Cuba de Geofabrik (unos 60 MB) y lo prepara para OSRM. En una máquina normal tarda menos de un minuto y usa hasta 1 GB de memoria mientras prepara. Los arranques siguientes reutilizan el mapa preparado.

No hace falta poner `OSRM_URL`, `DATABASE_URL` ni `STREETS_DB` en `.env`: el compose los fija para los contenedores.

`--build` compila el bot a partir del código de la carpeta (para probar cambios). En el servidor es mejor usar la imagen ya publicada, que no necesita compilar:

```bash
docker compose pull bot
docker compose up -d
```

## Imagen publicada

GitHub Actions (`.github/workflows/ci.yml`) pasa `cargo clippy` y `cargo test` en cada pull request y en cada push a `main`. Al crear una **etiqueta de versión** también construye la imagen del bot y la publica en `ghcr.io/forte11cuba/aserevoy`, solo si los tests pasan. No se publica nada en cada push: solo las versiones.

Para sacar una versión, sube antes `version` en `Cargo.toml` (la etiqueta debe coincidir, o el workflow falla) y luego:

```bash
git tag v0.1.0
git push origin v0.1.0
```

La versión `v0.1.0` publica las etiquetas `:0.1.0`, `:0.1` y `:latest`.

Por defecto el compose usa `:latest`. Para fijar una versión (y poder volver a una anterior), ponla en `.env`:

```bash
ASEREVOY_VERSION=0.1.0
```

y ejecuta `docker compose pull bot && docker compose up -d`.

La imagen no lleva secretos (`.env` queda fuera por `.dockerignore`) y es solo para `amd64`.

## Qué corre

| Servicio | Qué hace | Memoria |
|---|---|---|
| `osrm-download` | Descarga el mapa si no está; luego termina | — |
| `osrm-prepare` | Prepara el mapa si no está preparado; luego termina | hasta ~1 GB, solo la primera vez |
| `osrm` | Calcula rutas; accesible en `http://127.0.0.1:5000` solo desde esta máquina | ~210 MB |
| `bot` | El bot de Telegram | poca |

Prueba rápida de OSRM (de Coppelia al Capitolio, unos 3,5 km):

```bash
curl "http://127.0.0.1:5000/route/v1/driving/-82.3834,23.1396;-82.3597,23.1352?overview=false"
```

## Datos y copias de seguridad

Hay dos volúmenes:

- `aserevoy_bot-data`: **la base de datos del bot** (`aserevoy.db`). Es lo único que hay que respaldar.
- `aserevoy_osrm-data`: el mapa preparado (unos 370 MB). Se puede borrar sin perder nada: se vuelve a generar.

Copia manual de la base de datos. El bot se para unos segundos para que la copia sea consistente (SQLite usa el modo WAL, y copiar el archivo mientras escribe puede dejarla incompleta):

```bash
docker compose stop bot
docker run --rm -v aserevoy_bot-data:/data -v "$PWD":/out debian:bookworm-slim \
  cp /data/aserevoy.db /out/aserevoy-$(date +%F).db
docker compose start bot
```

Las copias diarias cifradas y fuera del servidor son de la fase 5.

## Actualizar el mapa

Cada pocos meses, para tener las calles nuevas y las correcciones de OpenStreetMap:

```bash
docker compose down
docker volume rm aserevoy_osrm-data
docker compose up -d
```

`data/zones.geojson` y `data/streets.sqlite` (municipios, calles y lugares para el canal y las direcciones escritas) van dentro de la imagen del bot. Para actualizarlos, vuelve a ejecutar `scripts/prepare_zones.py` y `scripts/prepare_osm.py` (ver sus cabeceras) y reconstruye con `docker compose up -d --build`.

## Sin token

Si `TELEGRAM_TOKEN` está vacío, el bot prepara la base de datos y termina; con `restart: unless-stopped`, Docker lo vuelve a arrancar cada pocos segundos. Pon el token o para el servicio con `docker compose stop bot`.

## Licencias

OSRM es software libre (BSD). Los datos del mapa son © colaboradores de OpenStreetMap, bajo la ODbL (ver `data/README.md`).
