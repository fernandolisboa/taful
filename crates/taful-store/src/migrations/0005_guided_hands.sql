-- D26: guided hands the user played to the end, per content version. A
-- lesson that lists one is done once its quiz is passed and the hand is
-- finished.

CREATE TABLE guided_hands (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    hand_id     TEXT NOT NULL,
    version     INTEGER NOT NULL,
    UNIQUE (hand_id, version)
);
