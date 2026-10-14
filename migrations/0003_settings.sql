-- Values the bot creates once and reuses across restarts (for example, the
-- join-request link to the offers channel).

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
