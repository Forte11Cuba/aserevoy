<p align="center">
  <img src="img/logo-amarillo-contorno.svg" alt="Asere Voy" width="360">
</p>

# Asere Voy

Bot de Telegram que conecta pasajeros y choferes de moto, carro o triciclo en La Habana. **Tú propones el precio**: el cliente dice cuánto paga, y los choferes aceptan o contraofertan, al estilo de inDrive.

Todo pasa en Telegram: no hay que instalar nada más. El pago es en CUP, directo al chofer (efectivo, transferencia o bitcoin); el bot no maneja dinero.

## Cómo funciona

Dentro del bot, el botón **❓ Cómo funciona** tiene estas mismas guías.

### Para el cliente

1. Pulsa **🚕 Pedir viaje**.
2. Marca la **recogida**: tu ubicación, un punto en el mapa, una dirección escrita ("23 e/ L y M, Vedado", "Calle L 23 y 21") o un lugar conocido ("Coppelia"). El bot te lo enseña en el mapa y confirmas con **"Sí, es aquí"**.
3. Marca el **destino** de la misma forma.
4. Si hay choferes de varios tipos, eliges **en qué quieres ir** (🏍 moto, 🚗 carro o 🛺 triciclo), cada uno con su precio sugerido; si solo hay de un tipo, el paso no sale. El bot calcula la distancia y te sugiere un precio. **Ajústalo** con −/+ 100, 1k, 5k o 10k (nunca por debajo del mínimo) y elige cómo pagas.
5. Revisa el resumen y **publica**. Tu solicitud sale en el canal de choferes, que solo ven el municipio y la esquina más cercana, nunca tu ubicación exacta.
6. Un chofer **acepta** tu precio o te manda una **contraoferta**. Ves su foto, su vehículo, su chapa, su nombre completo y su reputación, y lo **confirmas**.
7. Recibes su contacto y él recibe tu ubicación y lo que hayas querido compartir en **🔧 Ajustes → 👤 Mis datos** (nombre, celular, foto). Si no tienes celular guardado, al pedir el viaje el bot te ofrece ponerlo, o lo omites. Durante el viaje tienes **🚶 Ya voy**, **📤 Compartir viaje**, **🏁 Completado** y **❌ Cancelar viaje**.
8. Al terminar, **calificas** al chofer (y él a ti).

Mientras esperas puedes **subir el precio** o cancelar. Una solicitud dura hasta 60 minutos; a la media hora el bot te pregunta si sigues esperando.

### Para el chofer

1. En **🤝 Trabaja con nosotros → Aplicar a ser chofer**, completa tu ficha: alias, nombre completo, celular, tu vehículo (tipo, chapa, modelo y color) y fotos. Un admin la aprueba, junto con el vehículo.
2. Entra en el **canal de ofertas** con el enlace que te manda el bot.
3. Cada oferta dice en su primera línea para qué vehículo es; solo la puedes tomar si tu vehículo activo es de ese tipo (si tienes otro aprobado que sirve, el bot te ofrece cambiarlo de un toque). En cada oferta: **✅ Aceptar** el precio del cliente, o **💬 Contraofertar**, donde ajustas el monto con −/+ 100, 1k, 5k o 10k y pulsas **✅ Enviar**.
4. Cuando el cliente te confirma, recibes la recogida, el destino y su contacto. Marca **📍 Llegué** y **🏁 Completado**.
5. En **🧢 Chofer** tienes tus **📊 estadísticas**, tu **🪪 ficha**, que puedes editar campo por campo, y **🚘 Mis vehículos**: hasta 5 y uno activo, que cambias cuando quieras salvo con un viaje en curso o una contraoferta vigente. Un cambio de nombre completo, un vehículo nuevo y una chapa o foto nueva los revisa un admin.

### Para los admins

- **⚙️ Administración**: choferes (aprobar, bloquear, quitar, con sus vehículos), invitar choferes y, para los superadmins, admins, tarifas por tipo de vehículo y métricas.
- Las solicitudes de chofer, los vehículos nuevos, los cambios de nombre, chapa o foto y los reportes llegan por el bot con sus botones.

### Privacidad

- **Historial de 45 días:** cada uno ve sus viajes completados en 🧾 Mis viajes (lugares como texto, nunca coordenadas, cifrado) y puede borrarlo; pasados 45 días se borran solos. Las solicitudes activas se borran al cerrarse.
- De cada persona solo queda su reputación agregada (estrellas, viajes, días en el bot) y, si el cliente quiere, su nombre, celular y foto, que borra cuando quiera.
- Los celulares y los nombres se guardan cifrados.
- Los logs no tienen ubicaciones, direcciones ni teléfonos.

## Cómo está hecho

- **Rust** con [teloxide](https://github.com/teloxide/teloxide) (Telegram), [sqlx](https://github.com/launchbadge/sqlx) y SQLite.
- **OSRM** propio para calcular la distancia de cada viaje.
- **OpenStreetMap** para los municipios, repartos, calles, esquinas y lugares (`data/`), que permiten entender direcciones escritas.
- **Jev** (TypeSafe), opcional, para elegir entre varios lugares posibles de una dirección escrita.

El diseño completo, con el registro de decisiones, está en [`docs/diseno.md`](docs/diseno.md).

## Arrancarlo

Con Docker (el bot y su OSRM juntos; detalles en [`docs/docker.md`](docs/docker.md)):

```bash
cp .env.example .env      # y rellénalo
docker compose up -d --build
docker compose logs -f bot
```

Lo mínimo en `.env`: `TELEGRAM_TOKEN`, `SUPERADMIN_IDS`, `OFFERS_CHANNEL_ID` (el bot debe ser admin del canal), `PHONE_KEY` y `PAIR_KEY` (`openssl rand -hex 32` cada una). El resto está explicado en `.env.example`.

Para desarrollar:

```bash
cargo test
cargo clippy --all-targets
cargo run                 # necesita .env y un OSRM en OSRM_URL
```

## Licencia

El código es [MIT](LICENSE). Los datos del mapa son © colaboradores de [OpenStreetMap](https://www.openstreetmap.org/copyright), bajo la [ODbL](https://opendatacommons.org/licenses/odbl/1-0/) (ver [`data/README.md`](data/README.md)).
