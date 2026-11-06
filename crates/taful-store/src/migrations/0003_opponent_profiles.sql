-- #26: review items and leak buckets keyed by opponent profile (D8 amendment).
--
-- Spot-template keys move from v1 to v2, which appends the opponent segment
-- (`|<profile id or reference>`). Everything stored before this migration was
-- played against the reference bot. Rewritten rows get a new `updated_at`, so
-- a later sync (D9) sees the change.

-- The opponent of a decision with a context: 'reference' or a bot profile id.
ALTER TABLE decisions ADD COLUMN opponent TEXT;

UPDATE decisions
SET opponent = 'reference',
    template_key = CASE
        WHEN template_key GLOB 'v1|*' THEN 'v2|' || substr(template_key, 4) || '|reference'
        ELSE template_key
    END,
    -- `<template key>#<node>`
    node_key = CASE
        WHEN node_key GLOB 'v1|*#*' THEN 'v2|' || substr(node_key, 4, instr(node_key, '#') - 4)
            || '|reference' || substr(node_key, instr(node_key, '#'))
        ELSE node_key
    END,
    updated_at = max(updated_at, CAST(strftime('%s', 'now') AS INTEGER) * 1000)
WHERE spot IS NOT NULL;

UPDATE review_items
SET item_key = 'v2|' || substr(item_key, 4) || '|reference',
    updated_at = max(updated_at, CAST(strftime('%s', 'now') AS INTEGER) * 1000)
WHERE kind = 'spot_template' AND item_key GLOB 'v1|*';
