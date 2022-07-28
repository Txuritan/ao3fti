CREATE TABLE IF NOT EXISTS story_authors (
    story_id INTEGER NOT NULL,
    author_id INTEGER NOT NULL,
    created DATETIME NOT NULL DEFAULT (datetime(CURRENT_TIMESTAMP, 'utc')),
    PRIMARY KEY (story_id, author_id)
);
