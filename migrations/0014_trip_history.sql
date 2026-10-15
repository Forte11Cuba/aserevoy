-- Trip history (decision 110): each side of a completed trip keeps an entry
-- for 45 days. Places are text (municipality and corner, or what the client
-- wrote), never coordinates; texts and the other person are encrypted.
CREATE TABLE trip_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    telegram_id     INTEGER NOT NULL,
    role            TEXT    NOT NULL CHECK (role IN ('client', 'driver')),
    -- The user's trip number in that role when it was completed (1, 2, …).
    number          INTEGER NOT NULL,
    request_id      TEXT    NOT NULL,
    completed_at    INTEGER NOT NULL,
    origin_enc      BLOB    NOT NULL,
    destination_enc BLOB    NOT NULL,
    price           INTEGER NOT NULL,
    payment         TEXT    NOT NULL,
    counterpart_enc BLOB    NOT NULL,
    -- To record the stars this user gives once they rate.
    rating_code     TEXT,
    stars           INTEGER
);
CREATE INDEX trip_history_by_user ON trip_history (telegram_id, role, completed_at);

-- Drivers' totals per month (YYYY-MM, Cuban time), kept for good: no trip
-- details, only how many and how much (decision 111).
CREATE TABLE driver_monthly (
    telegram_id INTEGER NOT NULL,
    month       TEXT    NOT NULL,
    trips       INTEGER NOT NULL DEFAULT 0,
    earned      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (telegram_id, month)
);
