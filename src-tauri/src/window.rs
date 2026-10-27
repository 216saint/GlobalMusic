//! Janela principal. Criada em código (e não no tauri.conf.json) porque cada SO
//! tem seu próprio "vidro" nativo:
//! - Windows 10/11: Acrylic + barra de título própria;
//! - macOS: Vibrancy + semáforos nativos sobre o conteúdo;
//! - Linux: sem blur nativo confiável; o vidro vem do fundo ambiente desenhado na UI.

use tauri::{App, WebviewUrl, WebviewWindowBuilder};

/// Flags do WebView2 que cortam processos/serviços que um player não usa.
/// Mantém os defaults do Tauri (msWebOOUI, msPdfOOUI, msSmartScreenProtection).
///
/// ATENÇÃO: todas as janelas do app compartilham o mesmo perfil do WebView2, e o
/// WebView2 exige que todas sejam criadas com as MESMAS flags. Uma janela com flags
/// diferentes falha com `0x8007139F` ("recurso não está no estado correto").
/// Por isso as janelas de login (`auth::web_login`) usam esta mesma constante.
#[cfg(target_os = "windows")]
pub const WEBVIEW2_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,OptimizationHints,AutofillServerCommunication \
--disable-background-networking --disable-component-update --disable-sync --renderer-process-limit=1";

pub fn create_main(app: &App) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("GlobalMusic")
        .inner_size(1180.0, 760.0)
        .min_inner_size(860.0, 580.0)
        .center();

    #[cfg(target_os = "windows")]
    let builder = {
        use tauri::window::{Effect, EffectsBuilder};
        builder
            .decorations(false)
            .transparent(true)
            .effects(EffectsBuilder::new().effect(Effect::Acrylic).build())
            .additional_browser_args(WEBVIEW2_ARGS)
    };

    #[cfg(target_os = "macos")]
    let builder = {
        use tauri::TitleBarStyle;
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        builder
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .transparent(true)
            .effects(
                EffectsBuilder::new()
                    .effect(Effect::UnderWindowBackground)
                    .state(EffectState::FollowsWindowActiveState)
                    .build(),
            )
    };

    #[cfg(target_os = "linux")]
    let builder = builder.decorations(false);

    builder.build()?;
    Ok(())
}
