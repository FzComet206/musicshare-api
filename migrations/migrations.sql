CREATE TABLE Users (
    user_id SERIAL PRIMARY KEY,
    oauth_type VARCHAR(255),
    sub VARCHAR(255) UNIQUE,
    name VARCHAR(255),
    picture VARCHAR(255),
    number_of_files INTEGER DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_sub ON Users(sub);

CREATE TABLE Files (
    file_id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES Users(user_id) ON DELETE CASCADE,
    url VARCHAR(255) NOT NULL,
    uuid VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(255),
    name_tsv tsvector,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_files_name_tsv ON Files USING GIN(name_tsv);

CREATE TABLE Sessions (
    session_id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES Users(user_id) ON DELETE CASCADE,
    num_joined INTEGER DEFAULT 0,
    num_played INTEGER DEFAULT 0,
    start_date TIMESTAMP,
    end_date TIMESTAMP
);

CREATE INDEX idx_files_user_id ON Files(user_id);
CREATE INDEX idx_sessions_user_id ON Sessions(user_id);

CREATE OR REPLACE FUNCTION files_tsv_trigger() RETURNS trigger AS $$
BEGIN
    NEW.name_tsv := to_tsvector('english', COALESCE(NEW.name, ''));
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_files_tsv_update
BEFORE INSERT OR UPDATE OF name ON Files
FOR EACH ROW EXECUTE FUNCTION files_tsv_trigger();