-- Playlists do GlobalMusic compartilhadas por link: uma cópia congelada no momento do compartilhamento.
-- `tracks` é o JSON das faixas como o app mandou (o servidor não interpreta); `cover` é uma data URL JPEG.
CREATE TABLE shared_playlists (
    code       TEXT PRIMARY KEY,
    owner_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    tracks     TEXT NOT NULL,
    cover      TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX shared_playlists_owner ON shared_playlists(owner_id);
