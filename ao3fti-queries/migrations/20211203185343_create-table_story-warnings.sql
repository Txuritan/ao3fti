CREATE TABLE IF NOT EXISTS story_warnings (
    story_id INTEGER NOT NULL,
    warning_id INTEGER NOT NULL,
    created DATETIME NOT NULL DEFAULT (datetime(CURRENT_TIMESTAMP, 'utc')),
    PRIMARY KEY (story_id, warning_id)
);
