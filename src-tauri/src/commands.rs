//! Superfície IPC exposta ao frontend (wrappers tipados em `src/lib/api.ts`).

use std::time::Duration;

use futures::future::join_all;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::aggregate;
use crate::audio::{AudioEvent, PrepareReason};
use crate::imports::{self, Link};
use crate::local_playlists::LocalPlaylist;
use crate::lyrics::Lyrics;
use crate::error::{Error, Result};
use crate::models::{AccountStatus, Playlist, Source, SourceResults, Track};
use crate::player::queue::Slot;
use crate::player::routing::PlaybackRoute;
use crate::player::{Player, PlayerSnapshot};
use crate::state::AppState;

const DEFAULT_SEARCH_LIMIT: u32 = 10;
pub const PLAYER_EVENT: &str = "player://state";
/// Mensagem legível quando uma faixa não pôde ser tocada (a UI mostra e o player pula).
pub const PLAYER_ERROR_EVENT: &str = "player://error";
/// Aviso curto do player para a UI (ex.: "Pulei um trecho sem música").
pub const PLAYER_NOTICE_EVENT: &str = "player://notice";
/// Quantas faixas à frente da atual preparamos (ver `preload_upcoming`).
const LOOKAHEAD: usize = 3;
/// Destas, quantas são baixadas inteiras para o disco (as outras só têm a URL resolvida).
const PRELOAD_FULL: usize = 2;
/// De uma lista que apareceu na tela, quantas faixas do topo preparamos antes do clique.
const PREPARE_VISIBLE: usize = 4;

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String, limit: Option<u32>) -> Result<Vec<SourceResults<Track>>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(aggregate::search(&state.providers.all, &query, limit.unwrap_or(DEFAULT_SEARCH_LIMIT)).await)
}

#[tauri::command]
pub async fn library_playlists(state: State<'_, AppState>) -> Result<Vec<SourceResults<Playlist>>> {
    Ok(aggregate::library(&state.providers.all).await)
}

#[tauri::command]
pub async fn playlist_tracks(state: State<'_, AppState>, source: Source, playlist_id: String) -> Result<Vec<Track>> {
    // Playlist de outra pessoa, importada por link: no Spotify ela tem um caminho próprio.
    if source == Source::Spotify && state.imports.contains(source, &playlist_id).await {
        return match state.providers.spotify.shared_playlist_tracks(&playlist_id).await {
            // Sem conta do Spotify: lê a página pública da playlist (as faixas tocam pelo YouTube).
            Err(Error::NotConnected(_)) => imports::spotify_embed_tracks(&state.http, &playlist_id).await,
            other => other,
        };
    }
    state.providers.get(source).playlist_tracks(&playlist_id).await
}

#[tauri::command]
pub async fn accounts(state: State<'_, AppState>) -> Result<Vec<AccountStatus>> {
    let accounts = join_all(state.providers.all.iter().map(|p| p.account())).await;
    // Ao abrir o app já logado, é aqui que o player descobre se pode usar o librespot.
    if let Some(spotify) = accounts.iter().find(|a| a.source == Source::Spotify) {
        // Premium, mas o Spotify vem recusando o librespot: toca pelo YouTube (ver `note_native_refused`).
        let premium = spotify.premium.unwrap_or(false) && !state.audio.native_disabled();
        state.player.lock().await.set_spotify_premium(premium);
        if premium {
            warm_spotify(&state);
        }
    }
    Ok(accounts)
}

#[tauri::command]
pub async fn connect_account(app: AppHandle, state: State<'_, AppState>, source: Source) -> Result<AccountStatus> {
    let account = state.providers.get(source).connect(&app).await?;
    if source == Source::Spotify {
        let premium = account.premium.unwrap_or(false) && !state.audio.native_disabled();
        state.player.lock().await.set_spotify_premium(premium);
        if premium {
            warm_spotify(&state);
        }
    }
    Ok(account)
}

#[tauri::command]
pub async fn disconnect_account(state: State<'_, AppState>, source: Source) -> Result<()> {
    if source == Source::Spotify {
        state.player.lock().await.set_spotify_premium(false);
    }
    state.providers.get(source).disconnect().await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteResult {
    pub favorites: Vec<Track>,
    /// Curtiu no GlobalMusic, mas o serviço de origem recusou (ex.: precisa reconectar).
    pub remote_error: Option<String>,
}

#[tauri::command]
pub async fn favorites_list(state: State<'_, AppState>) -> Result<Vec<Track>> {
    Ok(state.favorites.list())
}

/// Curte/descurte no GlobalMusic **e** na conta da pessoa no serviço de origem.
#[tauri::command]
pub async fn set_favorite(state: State<'_, AppState>, track: Track, liked: bool) -> Result<FavoriteResult> {
    let favorites = state.favorites.set(&track, liked);
    let remote_error = match state.providers.get(track.source).set_liked(&track, liked).await {
        // Sem conta daquele serviço, a curtida fica só no GlobalMusic, e isso não é um erro.
        Ok(()) | Err(Error::NotConnected(_)) => None,
        Err(e) => {
            log::warn!("curtida no {} falhou para \"{}\": {e}", track.source.label(), track.title);
            Some(format!("{}: {e}", track.source.label()))
        }
    };
    Ok(FavoriteResult { favorites, remote_error })
}

/// Client ID próprio do Spotify (opcional): cota de requisições exclusiva da pessoa.
#[tauri::command]
pub async fn spotify_custom_client_id(state: State<'_, AppState>) -> Result<Option<String>> {
    Ok(state.providers.spotify.custom_client_id())
}

#[tauri::command]
pub async fn set_spotify_custom_client_id(state: State<'_, AppState>, client_id: Option<String>) -> Result<()> {
    state.providers.spotify.set_custom_client_id(client_id.as_deref()).await?;
    state.player.lock().await.set_spotify_premium(false);
    Ok(())
}

#[tauri::command]
pub async fn player_state(state: State<'_, AppState>) -> Result<PlayerSnapshot> {
    Ok(state.player.lock().await.snapshot())
}

#[tauri::command]
pub async fn player_play(
    app: AppHandle,
    state: State<'_, AppState>,
    tracks: Vec<Track>,
    start: usize,
    context_name: Option<String>,
) -> Result<PlayerSnapshot> {
    let before = state.player.lock().await.snapshot();
    let snapshot = update_player(&app, &state, |p| p.play_queue(tracks, start, context_name)).await;
    // Duplo clique numa faixa = dois "tocar". Se ela já está carregando, o carregamento
    // em andamento serve: recomeçar jogaria fora o yt-dlp e o download já feitos.
    let already_loading = before.loading
        && matches!((&before.current, &snapshot.current), (Some(a), Some(b)) if a.source == b.source && a.id == b.id);
    if !already_loading {
        start_current(&app, &snapshot).await;
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn player_toggle(app: AppHandle, state: State<'_, AppState>) -> Result<PlayerSnapshot> {
    let snapshot = update_player(&app, &state, Player::toggle).await;
    if !snapshot.playing {
        state.audio.pause().await;
    } else if state.audio.is_active().await {
        state.audio.resume().await;
    } else {
        // Nada carregado (ex.: pulou de faixa com o player pausado): carrega agora.
        start_current(&app, &snapshot).await;
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn player_next(app: AppHandle, state: State<'_, AppState>) -> Result<PlayerSnapshot> {
    let snapshot = update_player(&app, &state, Player::next).await;
    start_current(&app, &snapshot).await;
    Ok(snapshot)
}

#[tauri::command]
pub async fn player_previous(app: AppHandle, state: State<'_, AppState>) -> Result<PlayerSnapshot> {
    let snapshot = update_player(&app, &state, Player::previous).await;
    start_current(&app, &snapshot).await;
    Ok(snapshot)
}

/// "Adicionar à fila": entra no fim da sua fila (de qualquer fonte), antes do resto da playlist.
#[tauri::command]
pub async fn player_enqueue(app: AppHandle, state: State<'_, AppState>, track: Track) -> Result<PlayerSnapshot> {
    let mut started = false;
    let snapshot = update_player(&app, &state, |p| started = p.enqueue(track)).await;
    if started {
        start_current(&app, &snapshot).await;
    } else {
        // Entrou na fila: se estiver entre as próximas, já começa a carregar.
        preload_upcoming(&app).await;
    }
    Ok(snapshot)
}

/// "Tocar a seguir": entra no começo da sua fila, logo depois da faixa atual.
#[tauri::command]
pub async fn player_play_next(app: AppHandle, state: State<'_, AppState>, track: Track) -> Result<PlayerSnapshot> {
    let mut started = false;
    let snapshot = update_player(&app, &state, |p| started = p.play_next(track)).await;
    if started {
        start_current(&app, &snapshot).await;
    } else {
        // A "próxima" mudou: pré-carrega a nova.
        preload_upcoming(&app).await;
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn player_remove(app: AppHandle, state: State<'_, AppState>, slot: Slot) -> Result<PlayerSnapshot> {
    let snapshot = update_player(&app, &state, |p| {
        p.remove(slot);
    })
    .await;
    preload_upcoming(&app).await;
    Ok(snapshot)
}

#[tauri::command]
pub async fn player_jump(app: AppHandle, state: State<'_, AppState>, slot: Slot) -> Result<PlayerSnapshot> {
    let mut jumped = false;
    let snapshot = update_player(&app, &state, |p| jumped = p.jump(slot)).await;
    if jumped {
        start_current(&app, &snapshot).await;
    }
    Ok(snapshot)
}

/// Prepara (resolve a URL do áudio) antes do clique: as primeiras faixas de uma lista que
/// apareceu na tela (`hover = false`) ou a faixa em que o mouse parou (`hover = true`).
/// Volta na hora; o trabalho segue em segundo plano.
#[tauri::command]
pub async fn player_prepare(app: AppHandle, state: State<'_, AppState>, tracks: Vec<Track>, hover: bool) -> Result<()> {
    let (reason, limit) = if hover { (PrepareReason::Hover, 1) } else { (PrepareReason::Visible, PREPARE_VISIBLE) };
    let routed: Vec<_> = {
        let player = state.player.lock().await;
        tracks.into_iter().take(limit).map(|t| {
            let route = player.route(&t);
            (t, route)
        }).collect()
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        app.state::<AppState>().audio.prepare(routed, reason).await;
    });
    Ok(())
}

/// Playlists importadas por link (a UI junta com as da biblioteca).
#[tauri::command]
pub async fn imported_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>> {
    Ok(state.imports.list().await)
}

/// Importa a playlist de um link do Spotify, do YouTube Music ou do SoundCloud.
#[tauri::command]
pub async fn import_playlist(state: State<'_, AppState>, link: String) -> Result<Vec<Playlist>> {
    let playlist = match imports::parse_link(&link)? {
        Link::Spotify(id) => imports::describe(&state.http, Source::Spotify, &id).await?,
        Link::YoutubeMusic(id) => imports::describe(&state.http, Source::YoutubeMusic, &id).await?,
        Link::Soundcloud(url) => state.providers.soundcloud.resolve_playlist(&url).await?,
    };
    log::info!("playlist importada: \"{}\" ({})", playlist.name, playlist.source.label());
    Ok(state.imports.add(playlist).await)
}

#[tauri::command]
pub async fn remove_imported_playlist(state: State<'_, AppState>, source: Source, id: String) -> Result<Vec<Playlist>> {
    Ok(state.imports.remove(source, &id).await)
}

/// Playlists criadas no GlobalMusic (faixas de qualquer fonte, sem precisar de conta).
#[tauri::command]
pub async fn local_playlists(state: State<'_, AppState>) -> Result<Vec<LocalPlaylist>> {
    Ok(state.local_playlists.list())
}

#[tauri::command]
pub async fn create_local_playlist(state: State<'_, AppState>, name: String, tracks: Vec<Track>) -> Result<Vec<LocalPlaylist>> {
    state.local_playlists.create(&name, tracks)
}

#[tauri::command]
pub async fn add_to_local_playlist(state: State<'_, AppState>, id: String, tracks: Vec<Track>) -> Result<Vec<LocalPlaylist>> {
    state.local_playlists.add(&id, tracks)
}

#[tauri::command]
pub async fn remove_from_local_playlist(state: State<'_, AppState>, id: String, index: usize) -> Result<Vec<LocalPlaylist>> {
    state.local_playlists.remove_track(&id, index)
}

#[tauri::command]
pub async fn delete_local_playlist(state: State<'_, AppState>, id: String) -> Result<Vec<LocalPlaylist>> {
    state.local_playlists.delete(&id)
}

/// Letra da faixa (LRCLIB): sincronizada quando existir, senão corrida.
#[tauri::command]
pub async fn lyrics(state: State<'_, AppState>, track: Track) -> Result<Option<Lyrics>> {
    state.lyrics.get(&track).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alternatives {
    /// O vídeo que toca hoje no lugar da faixa (se já foi escolhido).
    pub current: Option<String>,
    pub options: Vec<Track>,
}

/// Versões no YouTube de uma faixa do Spotify, para a pessoa corrigir uma equivalência errada.
#[tauri::command]
pub async fn track_alternatives(state: State<'_, AppState>, track: Track) -> Result<Alternatives> {
    let (current, options) = state.audio.alternatives(&track).await?;
    Ok(Alternatives { current, options })
}

/// A pessoa escolheu a versão `video_id`. Se a faixa está tocando agora, troca na hora, do mesmo ponto.
#[tauri::command]
pub async fn choose_track_version(app: AppHandle, state: State<'_, AppState>, track: Track, video_id: String) -> Result<()> {
    state.audio.choose_version(&track, &video_id);
    let snapshot = state.player.lock().await.snapshot();
    let playing_it = snapshot.current.as_ref().is_some_and(|t| t.source == track.source && t.id == track.id);
    if playing_it && snapshot.route != Some(PlaybackRoute::SpotifyNative) {
        start_current(&app, &snapshot).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn player_seek(app: AppHandle, state: State<'_, AppState>, position_ms: u64) -> Result<PlayerSnapshot> {
    let snapshot = update_player(&app, &state, |p| p.seek(position_ms)).await;
    state.audio.seek(snapshot.position_ms).await;
    Ok(snapshot)
}

#[tauri::command]
pub async fn player_set_volume(app: AppHandle, state: State<'_, AppState>, volume: f32) -> Result<PlayerSnapshot> {
    let snapshot = update_player(&app, &state, |p| p.set_volume(volume)).await;
    state.audio.set_volume(snapshot.volume).await;
    Ok(snapshot)
}

/// Fim de faixa ou falha vindos do motor de áudio.
pub async fn on_audio_event(app: &AppHandle, event: AudioEvent) {
    let state = app.state::<AppState>();
    let generation = match &event {
        AudioEvent::Ended(g) | AudioEvent::Failed(g, _) | AudioEvent::NativeUnavailable(g) => *g,
    };
    // Evento de uma faixa que já não é a atual (a pessoa pulou antes): ignora.
    if !state.audio.is_current(generation) {
        return;
    }
    match event {
        AudioEvent::Ended(_) => {
            let snapshot = update_player(app, &state, Player::next).await;
            start_current(app, &snapshot).await;
        }
        // O Spotify recusou esta faixa: em vez de pular, tenta a mesma música pelo YouTube.
        AudioEvent::NativeUnavailable(_) => {
            let snapshot = state.player.lock().await.snapshot();
            if state.audio.note_native_refused() {
                log::warn!("o Spotify recusou o librespot; pelas próximas 24 h as faixas do Spotify vão direto pelo YouTube");
                // As próximas da fila passam a ir pelo YouTube: são pré-carregadas assim que esta tocar.
                state.player.lock().await.set_spotify_premium(false);
            }
            if let Some(track) = snapshot.current {
                log::info!("[{generation}] Spotify recusou \"{}\"; tentando pelo YouTube", track.title);
                // O relógio já andava (o librespot tinha aceitado a faixa): volta para "carregando", do zero.
                // Antes, a música começava em 2–3 s e esperava o download inteiro (começar do meio exige seek).
                update_player(app, &state, Player::reload).await;
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    match state.audio.load_fallback(generation, &track, 0, snapshot.volume).await {
                        Ok(()) if state.audio.is_current(generation) => {
                            update_player(&app, &state, |p| p.started(0)).await;
                            preload_upcoming(&app).await;
                            skip_offtopic(&app, generation, track, PlaybackRoute::YoutubeFallback);
                        }
                        Ok(()) => {}
                        Err(e) => state.audio.report(AudioEvent::Failed(generation, e.to_string())),
                    }
                });
            }
        }
        AudioEvent::Failed(_, reason) => {
            let keep_going = state.player.lock().await.failed();
            if keep_going {
                let _ = app.emit(PLAYER_ERROR_EVENT, format!("pulei uma faixa: {reason}"));
                let snapshot = update_player(app, &state, Player::next).await;
                start_current(app, &snapshot).await;
            } else {
                // Várias falhas seguidas indicam um problema geral (sem login, sem rede):
                // varrer a fila inteira só esconderia o motivo.
                let _ = app.emit(PLAYER_ERROR_EVENT, format!("parei depois de várias faixas falharem: {reason}"));
                state.audio.stop().await;
                update_player(app, &state, |_| {}).await;
            }
        }
    }
}

/// Abre a sessão do librespot em segundo plano (ver `Audio::warm_spotify`).
fn warm_spotify(state: &AppState) {
    let audio = state.audio.clone();
    tauri::async_runtime::spawn(async move { audio.warm_spotify().await });
}

/// Começa a tocar a faixa atual do snapshot em segundo plano.
/// O comando volta na hora (a UI responde já); resolver o áudio leva alguns segundos.
async fn start_current(app: &AppHandle, snapshot: &PlayerSnapshot) {
    let state = app.state::<AppState>();
    let generation = state.audio.begin();
    let (Some(track), Some(route), true) = (snapshot.current.clone(), snapshot.route, snapshot.playing) else {
        state.audio.stop().await;
        return;
    };
    let (start_ms, volume) = (snapshot.position_ms, snapshot.volume);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match state.audio.load(generation, track.clone(), route, start_ms, volume).await {
            // O som começou agora: o relógio passa a andar (a UI e o Discord não ficam adiantados).
            Ok(()) if state.audio.is_current(generation) => {
                let cancelled = !state.player.lock().await.is_loading();
                if cancelled {
                    // Pausou enquanto carregava: o áudio chegou, mas fica parado.
                    state.audio.pause().await;
                } else {
                    update_player(&app, &state, |p| p.started(start_ms)).await;
                    preload_upcoming(&app).await;
                    skip_offtopic(&app, generation, track, route);
                }
            }
            Ok(()) => {}
            // Pelo canal de eventos (e não chamando `on_audio_event` direto): as duas funções
            // async se chamariam mutuamente, e o tipo de uma conteria o da outra (recursão infinita de tipos).
            Err(e) => state.audio.report(AudioEvent::Failed(generation, e.to_string())),
        }
    });
}

/// Pula os trechos sem música (SponsorBlock) da faixa que acabou de começar, se o áudio vem do
/// YouTube. Acompanha a posição de no máximo 1 em 1 s, para seguir pausas e seeks da pessoa, e para
/// sozinho quando a faixa muda.
fn skip_offtopic(app: &AppHandle, generation: u64, track: Track, route: PlaybackRoute) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let Some(video_id) = state.audio.youtube_video_id(&track, route) else { return };
        let mut segments = state.sponsor.segments(&video_id).await.into_iter().peekable();
        while let Some(&(start, end)) = segments.peek() {
            if !state.audio.is_current(generation) {
                return;
            }
            let snapshot = state.player.lock().await.snapshot();
            let position = snapshot.position_ms;
            if position >= end {
                segments.next();
                continue;
            }
            if position >= start && snapshot.audible() {
                segments.next();
                let _ = app.emit(PLAYER_NOTICE_EVENT, "Pulei um trecho sem música");
                if end + 1_000 >= track.duration_ms {
                    // O trecho vai até o fim: é como se a música tivesse acabado.
                    state.audio.report(AudioEvent::Ended(generation));
                    return;
                }
                let snapshot = update_player(&app, &state, |p| p.seek(end)).await;
                state.audio.seek(snapshot.position_ms).await;
                continue;
            }
            let wait = start.saturating_sub(position).clamp(200, 1_000);
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }
    });
}

/// Prepara as próximas faixas em segundo plano, todas ao mesmo tempo: as `PRELOAD_FULL`
/// primeiras são baixadas inteiras (tocam na hora) e as outras têm a URL resolvida (pular até
/// elas não espera o yt-dlp). O yt-dlp em segundo plano tem vagas limitadas (`BACKGROUND_YTDLP`).
///
/// **Não é cancelado quando a pessoa pula de faixa.** Antes era, e pular para a faixa que estava
/// sendo preparada *matava* o yt-dlp dela no meio: o play começava tudo de novo (~5 s). Agora o
/// play aproveita o mesmo yt-dlp, e a pré-carga repetida da mesma faixa não baixa duas vezes.
async fn preload_upcoming(app: &AppHandle) {
    let state = app.state::<AppState>();
    let upcoming = state.player.lock().await.upcoming(LOOKAHEAD);
    for (i, (track, route)) in upcoming.into_iter().enumerate() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let audio = &app.state::<AppState>().audio;
            if i < PRELOAD_FULL {
                audio.preload(track, route).await;
            } else {
                audio.warm(&track, route).await;
            }
        });
    }
}

/// Aplica a mudança e avisa todas as janelas (atalhos de mídia do SO também passarão por aqui).
async fn update_player(app: &AppHandle, state: &AppState, change: impl FnOnce(&mut Player)) -> PlayerSnapshot {
    let snapshot = {
        let mut player = state.player.lock().await;
        change(&mut player);
        player.touch();
        player.snapshot()
    };
    state.presence.show(&snapshot);
    crate::media_keys::show(app, &snapshot);
    let _ = app.emit(PLAYER_EVENT, &snapshot);
    snapshot
}
