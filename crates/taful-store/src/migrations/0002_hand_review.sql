-- M5: full hands end to end, hand review, drills from mistakes, leaks.

-- The whole hand as data (holes, board, history), for replay.
ALTER TABLE hands ADD COLUMN replay TEXT;        -- JSON
ALTER TABLE hands ADD COLUMN hero_net REAL;     -- bb, once known

-- Decision context: spot template (review item key) and leak bucket.
ALTER TABLE decisions ADD COLUMN template_key TEXT;
ALTER TABLE decisions ADD COLUMN spot TEXT;      -- 'preflop' or a library configuration id
ALTER TABLE decisions ADD COLUMN street TEXT;    -- preflop | flop | turn | river
ALTER TABLE decisions ADD COLUMN node_type TEXT;
ALTER TABLE decisions ADD COLUMN texture TEXT;   -- texture class id, postflop only
ALTER TABLE decisions ADD COLUMN step INTEGER;   -- index into the hand's replay states

CREATE INDEX decisions_created ON decisions (created_at);
CREATE INDEX hands_created ON hands (created_at);
