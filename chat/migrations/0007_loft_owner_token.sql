-- Ejer-nøgle til at lukke et loft. Tilfældig UUID, oprettet ved POST /v1/lofts
-- og kun returneret til skaberen — deles aldrig i loft-linket.
ALTER TABLE lofts ADD COLUMN IF NOT EXISTS owner_token UUID;
