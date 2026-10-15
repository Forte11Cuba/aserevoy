-- The client's "a driver wants to take your trip" card, so it can be deleted
-- when it no longer applies (not confirmed in time, the driver withdrew).
ALTER TABLE requests ADD COLUMN confirm_msg_id INTEGER;
