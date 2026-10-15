-- When to suggest raising the price (5 minutes after publishing, once;
-- decision 112). NULL once suggested, or if the client raised it already.
ALTER TABLE requests ADD COLUMN nudge_at INTEGER;
