-- Phase 4: written addresses and free descriptions. A point without an exact
-- location is its municipality's centre (approx = 1) and the trip's km are
-- increased; the description goes to the driver after confirmation.

ALTER TABLE requests ADD COLUMN pickup_approx INTEGER NOT NULL DEFAULT 0;
ALTER TABLE requests ADD COLUMN dropoff_approx INTEGER NOT NULL DEFAULT 0;
-- The client's description of the destination (the pickup's is `reference`).
ALTER TABLE requests ADD COLUMN dropoff_note TEXT;
-- The suggested price when published, for the daily metrics (final / suggested).
ALTER TABLE requests ADD COLUMN suggested INTEGER NOT NULL DEFAULT 0;
