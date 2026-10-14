-- Drivers' full name (decisions 87 and 88), encrypted like the phone. Staff
-- see it to verify the driver; the client sees it once a trip is assigned.
-- A new name waits in `pending_full_name_enc` until an admin approves it.
-- Drivers from before have none until they add it from their profile.
ALTER TABLE drivers ADD COLUMN full_name_enc BLOB;
ALTER TABLE drivers ADD COLUMN pending_full_name_enc BLOB;
