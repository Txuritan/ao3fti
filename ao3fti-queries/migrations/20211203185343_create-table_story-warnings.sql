CREATE TABLE IF NOT EXISTS story_warnings (
    story_id SERIAL NOT NULL,
    warning_id SERIAL NOT NULL,
    created TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, warning_id)
);
