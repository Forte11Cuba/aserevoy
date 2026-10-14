-- Each counteroffer is its own message in the client's chat (decision 53),
-- so the single edited list goes away.

ALTER TABLE counteroffers ADD COLUMN client_msg_id INTEGER;
ALTER TABLE requests DROP COLUMN offers_msg_id;
