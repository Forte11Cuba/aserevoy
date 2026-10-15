-- Minutes of the trip by road, as OSRM estimated them, to show next to the
-- distance in the channel and the trip cards. 0 when unknown.
ALTER TABLE requests ADD COLUMN minutes REAL NOT NULL DEFAULT 0;
