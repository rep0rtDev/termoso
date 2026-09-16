-- Team activity digest: an admin asks to receive the team's activity log by
-- e-mail once a day or once a week. Nothing is sent to anyone who did not ask.
CREATE TABLE team_digests (
    team_id      uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id      uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cadence      text NOT NULL CHECK (cadence IN ('daily', 'weekly')),
    -- End of the last period that was mailed; nothing older is sent again.
    last_sent_at timestamptz NOT NULL DEFAULT now(),
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);
