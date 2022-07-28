CREATE TABLE IF NOT EXISTS story_origins (
    story_id INTEGER NOT NULL,
    origin_id INTEGER NOT NULL,
    created DATETIME NOT NULL DEFAULT (datetime(CURRENT_TIMESTAMP, 'utc')),
    PRIMARY KEY (story_id, origin_id)
);
