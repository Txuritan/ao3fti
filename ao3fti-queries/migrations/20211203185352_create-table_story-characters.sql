CREATE TABLE IF NOT EXISTS story_characters (
    story_id INTEGER NOT NULL,
    character_id INTEGER NOT NULL,
    created DATETIME NOT NULL DEFAULT (datetime(CURRENT_TIMESTAMP, 'utc')),
    PRIMARY KEY (story_id, character_id)
);
