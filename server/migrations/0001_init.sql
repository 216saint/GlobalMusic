-- Contas, sessões e amizades do GlobalMusic.

CREATE TABLE users (
    id           INTEGER PRIMARY KEY,
    -- @usuário: único, sem diferenciar maiúsculas; é por ele que amigos se acham.
    username     TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT    NOT NULL,
    -- URL externa (Discord/Spotify) ou foto enviada (servida em /avatars/...).
    avatar_url   TEXT,
    discord_id   TEXT    UNIQUE,
    spotify_id   TEXT    UNIQUE,
    created_at   INTEGER NOT NULL
);

-- Guardamos só o HASH do token: quem ler o banco não consegue usar as sessões.
CREATE TABLE sessions (
    token_hash TEXT    PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);
CREATE INDEX sessions_user ON sessions (user_id);

-- Um pedido de amizade é uma linha 'pending'; ao aceitar, vira 'accepted'.
CREATE TABLE friendships (
    requester_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    addressee_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    status       TEXT    NOT NULL CHECK (status IN ('pending', 'accepted')),
    created_at   INTEGER NOT NULL,
    PRIMARY KEY (requester_id, addressee_id),
    CHECK (requester_id <> addressee_id)
);
CREATE INDEX friendships_addressee ON friendships (addressee_id);
