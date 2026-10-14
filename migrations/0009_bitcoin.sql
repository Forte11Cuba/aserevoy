-- Bitcoin as a payment method (decision 74). SQLite cannot change a CHECK
-- constraint, so `requests` is rebuilt; the counteroffers are kept aside
-- because dropping `requests` cascades to them. Drivers no longer declare
-- payment methods (decision 75): `drivers.payment_methods` stays, unused.

CREATE TEMP TABLE counteroffers_keep AS SELECT * FROM counteroffers;

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
    payment        TEXT    NOT NULL CHECK (payment IN ('cash', 'transfer', 'bitcoin')),
    state          TEXT    NOT NULL
                   CHECK (state IN ('open', 'confirming', 'assigned', 'completed', 'cancelled', 'expired')),
    -- The driver holding the request ('confirming') or doing the trip ('assigned').
    driver_id      INTEGER,
    channel_msg_id INTEGER,
    created_at     INTEGER NOT NULL,
    assigned_at    INTEGER,
    -- When to ask "still waiting?"; NULL once asked.
    check_at       INTEGER,
    -- Deadline to answer "still waiting?"; NULL when no question is pending.
    answer_by      INTEGER,
    -- Deadline for the client to confirm the driver ('confirming' only).
    confirm_by     INTEGER,
    -- End of the request's life in the channel (60 minutes).
    expires_at     INTEGER NOT NULL,
    on_my_way_at   INTEGER,
    arrived_at     INTEGER,
    pickup_approx  INTEGER NOT NULL DEFAULT 0,
    dropoff_approx INTEGER NOT NULL DEFAULT 0,
    dropoff_note   TEXT,
    suggested      INTEGER NOT NULL DEFAULT 0
);

INSERT INTO requests_new SELECT id, client_id, client_phone, pickup_lat, pickup_lon, dropoff_lat,
    dropoff_lon, reference, km, price, client_price, payment, state, driver_id, channel_msg_id,
    created_at, assigned_at, check_at, answer_by, confirm_by, expires_at, on_my_way_at, arrived_at,
    pickup_approx, dropoff_approx, dropoff_note, suggested
FROM requests;

DROP TABLE requests;
ALTER TABLE requests_new RENAME TO requests;

CREATE INDEX requests_due ON requests (state, expires_at);
CREATE UNIQUE INDEX requests_one_per_driver ON requests (driver_id) WHERE state IN ('confirming', 'assigned');
CREATE UNIQUE INDEX requests_one_per_client ON requests (client_id) WHERE state IN ('open', 'confirming');

INSERT OR IGNORE INTO counteroffers SELECT * FROM counteroffers_keep;
DROP TABLE counteroffers_keep;
