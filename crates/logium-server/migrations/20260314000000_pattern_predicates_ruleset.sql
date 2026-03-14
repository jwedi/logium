-- Rename source_name to ruleset_name in pattern_predicates
ALTER TABLE pattern_predicates RENAME COLUMN source_name TO ruleset_name;

-- Migrate existing rows: resolve source_name → template → ruleset name.
-- For each predicate, find the source (by name, within same project as the pattern),
-- get its template_id, pick the first ruleset for that template.
-- Rows with no matching source/ruleset are left as-is (pattern simply won't fire).
UPDATE pattern_predicates
SET ruleset_name = (
    SELECT r.name
    FROM patterns p
    JOIN rulesets r ON r.template_id = (
        SELECT s.template_id FROM sources s
        WHERE s.project_id = p.project_id
          AND s.name = pattern_predicates.ruleset_name  -- still old value at this point
        LIMIT 1
    )
    WHERE p.id = pattern_predicates.pattern_id
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1 FROM patterns p WHERE p.id = pattern_predicates.pattern_id
);
