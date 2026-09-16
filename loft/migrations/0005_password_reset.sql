ALTER TABLE users ADD COLUMN reset_token UUID;
ALTER TABLE users ADD COLUMN reset_token_expires_at TIMESTAMPTZ;
