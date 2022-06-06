CREATE TABLE IF NOT EXISTS story_origins (
    story_id SERIAL NOT NULL,
    origin_id SERIAL NOT NULL,
    created TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, origin_id)
);
