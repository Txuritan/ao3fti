CREATE TABLE IF NOT EXISTS story_generals (
    story_id INTEGER NOT NULL,
    general_id INTEGER NOT NULL,
    created DATETIME NOT NULL DEFAULT (datetime(CURRENT_TIMESTAMP, 'utc')),
    PRIMARY KEY (story_id, general_id)
);
