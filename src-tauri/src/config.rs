//! Identificadores embutidos no app. Nenhum deles é segredo: são IDs públicos
//! que identificam o *aplicativo*, não o usuário. Cada pessoa continua entrando
//! com a própria conta — por isso ninguém precisa configurar nada para usar.
//! (Ver docs/decisoes/0003-login-sem-credenciais-de-desenvolvedor.md)

/// Porta do servidor de loopback que recebe o redirect do OAuth do Spotify.
pub const OAUTH_PORT: u16 = 47823;

pub mod spotify {
    use super::OAUTH_PORT;

    /// Client ID do app desktop do Spotify — o mesmo que o librespot usa.
    /// Não sofre o limite de usuários do "modo de desenvolvimento" dos apps
    /// registrados no dashboard. Decisão consciente do dono do projeto (ADR 0003).
    pub const CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

    pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
    pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
    pub const API_URL: &str = "https://api.spotify.com/v1";

    /// Esse cliente só aceita redirect em `http://127.0.0.1:<porta>/login`.
    pub fn redirect_uri() -> String {
        format!("http://127.0.0.1:{OAUTH_PORT}/login")
    }
}

/// Application ID do app "GlobalMusic" no Discord Developer Portal.
/// É público e é o mesmo para todo mundo que instalar o app.
/// `None` desliga o Rich Presence.
pub const DISCORD_APP_ID: Option<&str> = Some("1564155693282496582");

/// Servidor social do GlobalMusic (contas, perfis e amizades), na VPS do projeto.
/// Ver `server/` e docs/decisoes/0008-servidor-social.md.
pub const SOCIAL_API_URL: &str = "https://api.globalmusic.216saint.com.br";
