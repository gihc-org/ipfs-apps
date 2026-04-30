ALTER TABLE users ADD COLUMN IF NOT EXISTS email TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS verification_token UUID;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email
    ON users(email) WHERE email IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_users_verification_token
    ON users(verification_token) WHERE verification_token IS NOT NULL;
