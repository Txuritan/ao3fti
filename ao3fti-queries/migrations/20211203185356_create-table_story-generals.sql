CREATE TABLE IF NOT EXISTS story_generals (
    story_id SERIAL NOT NULL,
    general_id SERIAL NOT NULL,
    created TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, general_id)
);
