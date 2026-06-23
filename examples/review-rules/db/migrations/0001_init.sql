-- Sample migration. Changing any file under db/migrations/** makes the simple
-- `review/migrations` rule fire under `polint review <ref>`.
CREATE TABLE account (
    id INTEGER PRIMARY KEY,
    email TEXT NOT NULL UNIQUE
);
