-- Add is_dm flag to rooms table to distinguish DM rooms from public rooms
ALTER TABLE rooms ADD COLUMN IF NOT EXISTS is_dm BOOLEAN NOT NULL DEFAULT FALSE;

-- Create room_members junction table for DM membership tracking
-- This enables many-to-many relationship between users and DM rooms
CREATE TABLE IF NOT EXISTS room_members (
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (room_id, user_id)
);

-- Index for efficient "which rooms is this user a member of?" queries
CREATE INDEX IF NOT EXISTS idx_room_members_user ON room_members(user_id);

-- Index for efficient "who are the members of this room?" queries
CREATE INDEX IF NOT EXISTS idx_room_members_room ON room_members(room_id);

-- Index for filtering DM rooms in the public room list
-- Partial index only indexes rows where is_dm = TRUE, saving space
CREATE INDEX IF NOT EXISTS idx_rooms_is_dm ON rooms(is_dm) WHERE is_dm = TRUE;
