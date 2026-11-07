-- Vehicles (decisions 116 to 130): motorbikes, cars and tricycles. A driver
-- has up to five and works with one, the active one. A rejected vehicle is
-- deleted, so there are only two statuses.
CREATE TABLE vehicles (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    driver_id             INTEGER NOT NULL REFERENCES drivers (telegram_id) ON DELETE CASCADE,
    kind                  TEXT    NOT NULL CHECK (kind IN ('moto', 'car', 'tricycle')),
    plate                 TEXT    NOT NULL,
    model                 TEXT    NOT NULL,
    color                 TEXT    NOT NULL,
    photo_file_id         TEXT,
    status                TEXT    NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved')),
    -- A new plate or photo waiting for an admin; meanwhile the old ones count.
    pending_plate         TEXT,
    pending_photo_file_id TEXT,
    created_at            INTEGER NOT NULL,
    approved_by           INTEGER
);
CREATE INDEX vehicles_by_driver ON vehicles (driver_id);

-- NULL: no active vehicle (none yet, or the driver removed it).
ALTER TABLE drivers ADD COLUMN active_vehicle_id INTEGER REFERENCES vehicles (id) ON DELETE SET NULL;

-- The kind is fixed when the request is published; the vehicle, when a driver
-- reserves it or a counteroffer is taken. No foreign key: the cards fall back
-- to the kind if the vehicle is gone.
ALTER TABLE requests ADD COLUMN vehicle_kind TEXT NOT NULL DEFAULT 'moto'
    CHECK (vehicle_kind IN ('moto', 'car', 'tricycle'));
ALTER TABLE requests ADD COLUMN vehicle_id INTEGER;

ALTER TABLE trip_history ADD COLUMN vehicle_kind TEXT NOT NULL DEFAULT 'moto'
    CHECK (vehicle_kind IN ('moto', 'car', 'tricycle'));

-- Car and tricycle fares start as copies of the moto's; the moto keeps its
-- keys. A new database has no fares yet: the bot seeds all of them at start.
INSERT OR IGNORE INTO fares (key, value)
    SELECT 'car.' || key, value FROM fares WHERE key IN ('base', 'per_km', 'min_percent');
INSERT OR IGNORE INTO fares (key, value)
    SELECT 'tricycle.' || key, value FROM fares WHERE key IN ('base', 'per_km', 'min_percent');

-- Each driver's motorbike becomes their first vehicle and their active one.
-- It is approved if the driver ever was (active or blocked); a pending
-- application's bike is approved with it. The old columns stay unused.
INSERT INTO vehicles (driver_id, kind, plate, model, color, photo_file_id, status, created_at, approved_by)
    SELECT telegram_id, 'moto', plate, COALESCE(bike_model, ''), COALESCE(bike_color, ''), bike_photo_file_id,
           CASE WHEN status = 'pending' THEN 'pending' ELSE 'approved' END, created_at, approved_by
    FROM drivers
    WHERE plate IS NOT NULL AND plate != '';
UPDATE drivers SET active_vehicle_id = (SELECT id FROM vehicles WHERE vehicles.driver_id = drivers.telegram_id);
