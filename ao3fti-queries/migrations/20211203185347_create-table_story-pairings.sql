CREATE TABLE IF NOT EXISTS story_pairings (
    story_id SERIAL NOT NULL,
    pairing_id SERIAL NOT NULL,
    created TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
    PRIMARY KEY (story_id, pairing_id)
);
