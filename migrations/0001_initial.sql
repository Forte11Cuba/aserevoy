-- Asere Voy: initial schema (phase 1).
-- Timestamps are unix seconds (INTEGER). Temporary rows (requests, invitations)
-- are deleted when they close or expire; there is no trip history.

CREATE TABLE drivers (
    telegram_id        INTEGER PRIMARY KEY,
    alias              TEXT    NOT NULL,
    phone_enc          BLOB,
    plate              TEXT,
    bike_model         TEXT,
    bike_color         TEXT,
    photo_file_id      TEXT,
    bike_photo_file_id TEXT,
    payment_methods    TEXT    NOT NULL DEFAULT 'cash',
    status             TEXT    NOT NULL DEFAULT 'pending'
                       CHECK (status IN ('pending', 'active', 'blocked')),
    created_at         INTEGER NOT NULL,
    added_by           INTEGER,
    approved_by        INTEGER
);

-- Superadmins live only in .env; this table holds admins they appoint.
CREATE TABLE admins (
    telegram_id  INTEGER PRIMARY KEY,
    appointed_by INTEGER NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE invitations (
    code       TEXT    PRIMARY KEY,
    role       TEXT    NOT NULL CHECK (role IN ('driver')),
    created_by INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_by    INTEGER
);

CREATE TABLE requests (
    id             TEXT    PRIMARY KEY,
    client_id      INTEGER NOT NULL,
    client_phone   TEXT,
    pickup_lat     REAL    NOT NULL,
    pickup_lon     REAL    NOT NULL,
    dropoff_lat    REAL    NOT NULL,
    dropoff_lon    REAL    NOT NULL,
    reference      TEXT,
    km             REAL    NOT NULL,
    price          INTEGER NOT NULL,
    -- Reserved for v2 (brokers): what the end client pays. NULL in the MVP.
    client_price   INTEGER,
    payment        TEXT    NOT NULL CHECK (payment IN ('cash', 'transfer')),
    state          TEXT    NOT NULL
                   CHECK (state IN ('open', 'assigned', 'completed', 'cancelled', 'expired')),
    driver_id      INTEGER,
    channel_msg_id INTEGER,
    created_at     INTEGER NOT NULL,
    assigned_at    INTEGER,
    expires_at     INTEGER NOT NULL
);

CREATE INDEX requests_due ON requests (state, expires_at);
-- A driver can hold at most one assigned trip; a client at most one open request.
CREATE UNIQUE INDEX requests_one_assigned_per_driver ON requests (driver_id) WHERE state = 'assigned';
CREATE UNIQUE INDEX requests_one_open_per_client ON requests (client_id) WHERE state = 'open';

CREATE TABLE blocked (
    telegram_id INTEGER NOT NULL,
    role        TEXT    NOT NULL CHECK (role IN ('client', 'driver')),
    reason      TEXT,
    until       INTEGER, -- NULL = permanent
    created_at  INTEGER NOT NULL,
    blocked_by  INTEGER NOT NULL,
    PRIMARY KEY (telegram_id, role)
);

CREATE TABLE fares (
    key   TEXT    PRIMARY KEY,
    value INTEGER NOT NULL
);

CREATE TABLE user_prefs (
    telegram_id INTEGER PRIMARY KEY,
    lang        TEXT NOT NULL CHECK (lang IN ('es', 'en'))
);
