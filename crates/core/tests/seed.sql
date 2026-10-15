-- Demo data for the backend integration test and for trying the app locally.
--   createdb table_plus_rs_demo && psql table_plus_rs_demo -f crates/core/tests/seed.sql
CREATE TYPE mood AS ENUM ('happy', 'sad', 'neutral');

CREATE TABLE users (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name       text NOT NULL,
    email      text NOT NULL UNIQUE,
    mood       mood DEFAULT 'neutral',
    active     boolean NOT NULL DEFAULT true,
    bio        text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz
);
COMMENT ON TABLE users IS 'Application users';
COMMENT ON COLUMN users.email IS 'Login email';

CREATE FUNCTION touch_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN NEW.updated_at := now(); RETURN NEW; END $$;
CREATE TRIGGER users_touch BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

INSERT INTO users (name, email, mood, active, bio, created_at)
SELECT 'User ' || g, 'user' || g || '@example.com',
       (ARRAY['happy','sad','neutral'])[1 + g % 3]::mood,
       g % 7 <> 0,
       CASE WHEN g % 5 = 0 THEN NULL ELSE 'Bio for user ' || g END,
       now() - (g || ' hours')::interval
FROM generate_series(1, 1200) g;

CREATE TABLE posts (
    id        serial PRIMARY KEY,
    user_id   bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title     text NOT NULL,
    body      text,
    score     numeric(10,2) NOT NULL DEFAULT 0 CHECK (score >= 0),
    published boolean NOT NULL DEFAULT false,
    tags      text[],
    meta      jsonb
);
CREATE INDEX posts_user_idx ON posts (user_id);
CREATE INDEX posts_published_idx ON posts (user_id) WHERE published;

INSERT INTO posts (user_id, title, body, score, published, tags, meta)
SELECT 1 + g % 1200, 'Post number ' || g, 'Body text of post ' || g, (g % 100) / 4.0, g % 3 = 0,
       ARRAY['news', 'tag' || g % 5], jsonb_build_object('views', g * 3, 'featured', g % 10 = 0)
FROM generate_series(1, 3000) g;

-- No primary key on purpose: edits to this table use the row's ctid.
CREATE TABLE logs (
    level   text NOT NULL,
    message text,
    at      timestamptz NOT NULL DEFAULT now()
);
INSERT INTO logs (level, message) SELECT 'info', 'Boot message ' || g FROM generate_series(1, 25) g;

CREATE VIEW active_users AS SELECT id, name, email FROM users WHERE active;
CREATE MATERIALIZED VIEW post_counts AS SELECT user_id, count(*) AS posts FROM posts GROUP BY user_id;
CREATE FUNCTION add_one(i int) RETURNS int LANGUAGE sql IMMUTABLE AS 'SELECT i + 1';
CREATE SEQUENCE order_seq START 1000;

ANALYZE;
