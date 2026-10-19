-- Last run of every background job, for the admin console.
CREATE TABLE job_runs (
    name             text PRIMARY KEY,
    last_started_at  timestamptz NOT NULL DEFAULT now(),
    last_finished_at timestamptz,
    last_result      bigint,
    last_error       text
);
