-- What the bot is waiting for in each private chat (a ride being requested,
-- a driver profile being filled in). Short-lived: deleted when the
-- conversation ends, or by the sweeper after it sits idle.

CREATE TABLE dialogs (
    telegram_id INTEGER PRIMARY KEY,
    state       TEXT    NOT NULL, -- JSON
    updated_at  INTEGER NOT NULL
);

CREATE INDEX dialogs_updated ON dialogs (updated_at);
