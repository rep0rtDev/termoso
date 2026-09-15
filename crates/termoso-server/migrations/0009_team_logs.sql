-- Team session logs: members of a team vault see each other's recordings.
ALTER TABLE vaults ADD COLUMN session_logging boolean NOT NULL DEFAULT false;
ALTER TABLE vaults ADD COLUMN logs_seq bigint NOT NULL DEFAULT 0;

ALTER TABLE session_logs ADD COLUMN vault_seq bigint NOT NULL DEFAULT 0;
ALTER TABLE session_logs ADD COLUMN pinned boolean NOT NULL DEFAULT false;
ALTER TABLE session_logs ADD COLUMN note text NOT NULL DEFAULT '';
ALTER TABLE session_logs ADD COLUMN note_by uuid REFERENCES users(id) ON DELETE SET NULL;

WITH numbered AS (
    SELECT id, row_number() OVER (PARTITION BY vault_id ORDER BY seq, created_at) AS n
    FROM session_logs
)
UPDATE session_logs s SET vault_seq = numbered.n FROM numbered WHERE s.id = numbered.id;

UPDATE vaults v SET logs_seq = COALESCE(
    (SELECT max(vault_seq) FROM session_logs s WHERE s.vault_id = v.id), 0);

CREATE INDEX session_logs_vault_seq_idx ON session_logs (vault_id, vault_seq);
