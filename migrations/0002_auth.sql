CREATE TABLE user (
    id      TEXT PRIMARY KEY,
    name    TEXT NOT NULL,
    created TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE passkey (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id   TEXT NOT NULL REFERENCES user(id),
    name      TEXT NOT NULL,
    data      TEXT NOT NULL,
    created   TEXT NOT NULL DEFAULT (datetime('now')),
    last_used TEXT
);

CREATE TABLE signing_key (
    id      INTEGER PRIMARY KEY CHECK (id = 1),
    secret  BLOB NOT NULL,
    created TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE auth_challenge (
    id         TEXT PRIMARY KEY,
    state      TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('registration', 'authentication')),
    created    TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL
);
