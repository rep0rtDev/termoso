-- Profile pictures. The bytes live in their own table so user rows stay
-- small; `users.avatar_tag` mirrors the content tag clients use as the
-- cache key (NULL = no picture) so member lists need no join.
ALTER TABLE users ADD COLUMN avatar_tag text;

CREATE TABLE user_avatars (
    user_id    uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    tag        text NOT NULL,
    -- Already normalised: small square lossy WebP, a few KiB.
    image      bytea NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
