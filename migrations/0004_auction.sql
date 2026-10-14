-- Phase 2 (auction): requests live up to 60 minutes with a "still waiting?"
-- check, a driver's acceptance waits for the client's confirmation
-- ('confirming'), and drivers can make counteroffers.
-- SQLite cannot change a CHECK constraint, so `requests` is rebuilt.

CREATE TABLE requests_new (
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
                   CHECK (state IN ('open', 'confirming', 'assigned', 'completed', 'cancelled', 'expired')),
    -- The driver holding the request ('confirming') or doing the trip ('assigned').
    driver_id      INTEGER,
    channel_msg_id INTEGER,
    -- The client's message listing the counteroffers.
    offers_msg_id  INTEGER,
    created_at     INTEGER NOT NULL,
    assigned_at    INTEGER,
    -- When to ask "still waiting?"; NULL once asked.
    check_at       INTEGER,
    -- Deadline to answer "still waiting?"; NULL when no question is pending.
    answer_by      INTEGER,
    -- Deadline for the client to confirm the driver ('confirming' only).
    confirm_by     INTEGER,
    -- End of the request's life in the channel (60 minutes).
    expires_at     INTEGER NOT NULL
);

INSERT INTO requests_new (id, client_id, client_phone, pickup_lat, pickup_lon, dropoff_lat,
    dropoff_lon, reference, km, price, client_price, payment, state, driver_id, channel_msg_id,
    created_at, assigned_at, expires_at)
SELECT id, client_id, client_phone, pickup_lat, pickup_lon, dropoff_lat,
    dropoff_lon, reference, km, price, client_price, payment, state, driver_id, channel_msg_id,
    created_at, assigned_at, expires_at
FROM requests;

DROP TABLE requests;
ALTER TABLE requests_new RENAME TO requests;

CREATE INDEX requests_due ON requests (state, expires_at);
-- A driver holds at most one request at a time (waiting for confirmation or
-- on a trip); a client has at most one request in the channel.
CREATE UNIQUE INDEX requests_one_per_driver ON requests (driver_id) WHERE state IN ('confirming', 'assigned');
CREATE UNIQUE INDEX requests_one_per_client ON requests (client_id) WHERE state IN ('open', 'confirming');

-- A driver's latest counteroffer on a request; a new one replaces it.
CREATE TABLE counteroffers (
    request_id TEXT    NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    driver_id  INTEGER NOT NULL,
    amount     INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    PRIMARY KEY (request_id, driver_id)
);

CREATE INDEX counteroffers_by_driver ON counteroffers (driver_id);
