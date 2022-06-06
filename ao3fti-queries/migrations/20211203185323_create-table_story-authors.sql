CREATE TABLE IF NOT EXISTS story_authors (
    story_id SERIAL NOT NULL,
    author_id SERIAL NOT NULL,
    created TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, author_id)
);
