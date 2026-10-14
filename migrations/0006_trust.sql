-- Phase 3 (trust): on-my-way / arrived times, cancellations, blind ratings,
-- aggregate reputation, support codes, admin invitations and terms.

-- When the client tapped "On my way" and the driver "Arrived"; they decide
-- whom a cancellation counts against.
ALTER TABLE requests ADD COLUMN on_my_way_at INTEGER;
ALTER TABLE requests ADD COLUMN arrived_at INTEGER;

-- Cancellations that count against a person. Kept 30 days.
CREATE TABLE cancellations (
    telegram_id INTEGER NOT NULL,
    role        TEXT    NOT NULL CHECK (role IN ('client', 'driver')),
    created_at  INTEGER NOT NULL
);
CREATE INDEX cancellations_by_user ON cancellations (telegram_id, role, created_at);

-- Aggregates only: never which trip, with whom or where.
CREATE TABLE reputation (
    telegram_id     INTEGER NOT NULL,
    role            TEXT    NOT NULL CHECK (role IN ('client', 'driver')),
    first_use       INTEGER NOT NULL,
    trips           INTEGER NOT NULL DEFAULT 0,
    stars_sum       INTEGER NOT NULL DEFAULT 0,
    stars_count     INTEGER NOT NULL DEFAULT 0,
    tag_punctual    INTEGER NOT NULL DEFAULT 0,
    tag_kind        INTEGER NOT NULL DEFAULT 0,
    tag_overcharged INTEGER NOT NULL DEFAULT 0,
    tag_no_show     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (telegram_id, role)
);

-- Blind ratings of one finished trip, until both sides rated or 24 h passed.
-- The code identifies it in buttons; no trip data is kept.
CREATE TABLE pending_ratings (
    code          TEXT    PRIMARY KEY,
    client_id     INTEGER NOT NULL,
    driver_id     INTEGER NOT NULL,
    -- Whether this pair's rating counts (one per pair per week).
    counts        INTEGER NOT NULL,
    -- The client's rating of the driver, and the driver's of the client.
    client_stars  INTEGER,
    client_tags   INTEGER NOT NULL DEFAULT 0,
    client_done   INTEGER NOT NULL DEFAULT 0,
    driver_stars  INTEGER,
    driver_tags   INTEGER NOT NULL DEFAULT 0,
    driver_done   INTEGER NOT NULL DEFAULT 0,
    expires_at    INTEGER NOT NULL
);

-- HMAC of a client-driver pair, so the same two people cannot farm ratings.
CREATE TABLE recent_pairs (
    pair_hmac  TEXT    PRIMARY KEY,
    expires_at INTEGER NOT NULL
);

-- A short code a person gives the human support account, so admins can find
-- them in the bot (block, unblock, remove a driver) without their Telegram id.
-- Renewed after 30 days.
CREATE TABLE support_codes (
    telegram_id INTEGER PRIMARY KEY,
    code        TEXT    NOT NULL UNIQUE,
    created_at  INTEGER NOT NULL
);

-- Which version of the terms each person accepted.
CREATE TABLE terms_accepted (
    telegram_id INTEGER PRIMARY KEY,
    version     INTEGER NOT NULL,
    accepted_at INTEGER NOT NULL
);

-- Invitations can now also appoint admins.
CREATE TABLE invitations_new (
    code       TEXT    PRIMARY KEY,
    role       TEXT    NOT NULL CHECK (role IN ('driver', 'admin')),
    created_by INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_by    INTEGER
);
INSERT INTO invitations_new SELECT code, role, created_by, expires_at, used_by FROM invitations;
DROP TABLE invitations;
ALTER TABLE invitations_new RENAME TO invitations;
