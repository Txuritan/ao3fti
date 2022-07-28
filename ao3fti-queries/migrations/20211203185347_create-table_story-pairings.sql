CREATE TABLE IF NOT EXISTS story_pairings (
    story_id INTEGER NOT NULL,
    pairing_id INTEGER NOT NULL,
    created DATETIME NOT NULL DEFAULT (datetime(CURRENT_TIMESTAMP, 'utc')),
    PRIMARY KEY (story_id, pairing_id)
);
