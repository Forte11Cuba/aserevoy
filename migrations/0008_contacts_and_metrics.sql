-- Phase 4: driver invitations for a contact the admin picked, and daily
-- metrics (aggregates only, no ids).

-- The Telegram user a contact invitation is for (NULL for link invitations).
ALTER TABLE invitations ADD COLUMN for_user INTEGER;

-- One row per day (Cuban date, YYYY-MM-DD). Sums, so averages are sum / count.
CREATE TABLE daily_metrics (
    day           TEXT    PRIMARY KEY,
    published     INTEGER NOT NULL DEFAULT 0,
    assigned      INTEGER NOT NULL DEFAULT 0,
    completed     INTEGER NOT NULL DEFAULT 0,
    cancelled     INTEGER NOT NULL DEFAULT 0,
    expired       INTEGER NOT NULL DEFAULT 0,
    km_sum        REAL    NOT NULL DEFAULT 0,
    price_sum     INTEGER NOT NULL DEFAULT 0,
    -- Over assigned trips: final price and the suggested one, for the ratio.
    final_sum     INTEGER NOT NULL DEFAULT 0,
    suggested_sum INTEGER NOT NULL DEFAULT 0
);
