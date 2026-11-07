-- #51: flops dealt per postflop configuration, so the app deals every
-- solved flop once before repeating one. A configuration's current cycle is
-- its largest `cycle`; a new cycle starts once every solved flop was dealt.

CREATE TABLE dealt_flops (
    id          TEXT PRIMARY KEY,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    -- Short game-config key and spot id (`btn-bb-srp`).
    config_key  TEXT NOT NULL,
    spot        TEXT NOT NULL,
    cycle       INTEGER NOT NULL,
    -- Canonical flop, e.g. `Tc9c6d`.
    flop        TEXT NOT NULL,
    UNIQUE (config_key, spot, cycle, flop)
);
