-- Schema v1: the first-slice tables (spec §14.2). Never edit after release;
-- add 0002_*.sql instead. Rules (§14.1, D9): ULID text primary key, Unix-ms
-- timestamps, nullable deleted_at for soft deletes.

CREATE TABLE settings (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    key         TEXT NOT NULL UNIQUE,
    value       TEXT NOT NULL            -- JSON
);

CREATE TABLE lesson_progress (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    lesson_id   TEXT NOT NULL,
    version     INTEGER NOT NULL,
    state       TEXT NOT NULL CHECK (state IN ('locked', 'available', 'done')),
    best_score  REAL,                    -- 0..1, best quiz score at this version
    UNIQUE (lesson_id, version)
);

CREATE TABLE quiz_attempts (
    id           TEXT PRIMARY KEY,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    deleted_at   INTEGER,
    quiz_id      TEXT NOT NULL,
    version      INTEGER NOT NULL,
    answers      TEXT NOT NULL,          -- JSON
    score        REAL NOT NULL,          -- 0..1
    duration_ms  INTEGER NOT NULL
);
CREATE INDEX quiz_attempts_quiz ON quiz_attempts (quiz_id, version);

CREATE TABLE calc_attempts (
    id             TEXT PRIMARY KEY,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER,
    exercise       TEXT NOT NULL,        -- exercise kind id
    band           TEXT NOT NULL,
    params         TEXT NOT NULL,        -- JSON: the generated exercise
    answer         REAL,                 -- NULL when skipped
    correct_value  REAL NOT NULL,
    correct        INTEGER NOT NULL,
    duration_ms    INTEGER NOT NULL,
    rating         TEXT NOT NULL
);
CREATE INDEX calc_attempts_exercise ON calc_attempts (exercise, created_at);

CREATE TABLE hands (
    id                      TEXT PRIMARY KEY,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER,
    mode                    TEXT NOT NULL,   -- spot | full_hand
    config_key              TEXT NOT NULL,
    config_toml             TEXT NOT NULL,
    seed                    INTEGER NOT NULL,
    hero_seat               INTEGER NOT NULL,
    bot_ids                 TEXT NOT NULL,   -- JSON array
    library_format_version  INTEGER,
    ended_reason            TEXT,
    total_ev_loss           REAL
);

CREATE TABLE decisions (
    id                  TEXT PRIMARY KEY,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    deleted_at          INTEGER,
    hand_id             TEXT REFERENCES hands (id),  -- NULL in spot mode
    node_key            TEXT NOT NULL,
    hero_combo          TEXT NOT NULL,
    chosen_action       TEXT NOT NULL,
    ev_per_action       TEXT NOT NULL,   -- JSON
    frequencies         TEXT NOT NULL,   -- JSON
    pot                 REAL NOT NULL,
    ev_loss             REAL NOT NULL,
    grade               TEXT NOT NULL,
    thresholds_version  INTEGER NOT NULL
);
CREATE INDEX decisions_hand ON decisions (hand_id);

CREATE TABLE review_items (
    id               TEXT PRIMARY KEY,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    kind             TEXT NOT NULL CHECK (kind IN ('spot_template', 'calc_exercise', 'lesson_quiz')),
    item_key         TEXT NOT NULL,      -- template key / exercise key / quiz question key
    content_version  INTEGER,            -- lesson/quiz version the item points at (§12.4)
    stability        REAL,               -- FSRS memory state; NULL before the first review
    difficulty       REAL,
    due              INTEGER NOT NULL,
    last_review      INTEGER,
    reps             INTEGER NOT NULL DEFAULT 0,
    lapses           INTEGER NOT NULL DEFAULT 0,
    UNIQUE (kind, item_key)
);
CREATE INDEX review_items_due ON review_items (due) WHERE deleted_at IS NULL;

CREATE TABLE review_log (
    id            TEXT PRIMARY KEY,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    item_id       TEXT NOT NULL REFERENCES review_items (id),
    reviewed_at   INTEGER NOT NULL,
    rating        TEXT NOT NULL,
    elapsed_days  REAL NOT NULL,
    source_kind   TEXT,                  -- decision | calc_attempt | quiz_attempt
    source_id     TEXT
);
CREATE INDEX review_log_item ON review_log (item_id, reviewed_at);

CREATE TABLE layouts (
    id              TEXT PRIMARY KEY,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER,
    name            TEXT NOT NULL,
    schema_version  INTEGER NOT NULL,
    json            TEXT NOT NULL
);

CREATE TABLE themes (
    id              TEXT PRIMARY KEY,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER,
    name            TEXT NOT NULL,
    schema_version  INTEGER NOT NULL,
    json            TEXT NOT NULL
);

CREATE TABLE solve_candidates (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    config_key  TEXT NOT NULL,
    line        TEXT NOT NULL,
    history     TEXT NOT NULL,
    count       INTEGER NOT NULL,
    last_seen   INTEGER NOT NULL,
    UNIQUE (config_key, line, history)
);
