-- Nome e foto de cada login vinculado (Discord, Spotify), para a pessoa escolher qual usar no perfil.
-- Atualizados a cada login ou vínculo; o perfil em si (display_name/avatar_url) só muda quando ela escolhe.
ALTER TABLE users ADD COLUMN discord_name TEXT;
ALTER TABLE users ADD COLUMN discord_avatar TEXT;
ALTER TABLE users ADD COLUMN spotify_name TEXT;
ALTER TABLE users ADD COLUMN spotify_avatar TEXT;
