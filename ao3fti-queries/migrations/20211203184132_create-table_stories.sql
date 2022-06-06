CREATE TABLE IF NOT EXISTS stories (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    summary TEXT NOT NULL,
    rating TEXT NOT NULL
);
