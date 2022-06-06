CREATE TABLE IF NOT EXISTS story_characters (
    story_id SERIAL NOT NULL,
    character_id SERIAL NOT NULL,
    created TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, character_id)
);
