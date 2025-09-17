use anyhow::anyhow;
use crate::engine::pipeline::css::CssPipeline;
use crate::engine::pipeline::font::FontPipeline;
use crate::engine::pipeline::html::HtmlPipeline;
use crate::engine::pipeline::image::ImagePipeline;
use crate::engine::pipeline::js::JsPipeline;
use crate::engine::UaPolicy;
use crate::html::{DummyDocument, DummyHtml5Config};
use crate::net::decision::types::BlockReason;
use crate::net::{decide_handling, HandlingDecision, RenderTarget, RequestDestination};
use crate::net::loader::Document;
use crate::net::types::{FetchResult, FetchResultMeta, NavigationResult};

/// Hooks are functions that allows the router to call the correct pipeline for each type of
/// resource.
pub struct Hooks<'a> {
    pub html: &'a mut dyn HtmlPipeline,
    pub css: &'a mut dyn CssPipeline,
    pub js: &'a mut dyn JsPipeline,
    pub images: &'a mut dyn ImagePipeline,
    pub fonts: &'a mut dyn FontPipeline,
    // pub viewer: &'a mut dyn ViewerPipeline,
    // pub download: &'a mut dyn DownloadManager,
    // pub external: &'a mut dyn ExternalOpener,
}

pub enum RoutedOutcome {
    MainDocument(DummyDocument),
    ViewerRendered,
    DownloadStarted(std::path::PathBuf),
    DownloadFinished(std::path::PathBuf),

    CssLoaded(u64 /* id */),
    ScriptExecuted(u64 /* id */),
    ImageDecoded(u64 /* id */),
    FontLoaded(u64 /* id */),

    Blocked(BlockReason),
    Failed(anyhow::Error),
    Cancelled,
}

pub async fn route_response_for(
    dest: RequestDestination,
    meta: FetchResultMeta,
    fetch_result: FetchResult,
    policy: &*UaPolicy,
    hooks: &mut Hooks,
) -> RoutedOutcome {

    // Find the peek, either from the stream, or directly from the buffered body
    let peek = match fetch_result {
        FetchResult::Stream { peek, .. } => peek.as_slice(),
        FetchResult::Buffered { body, .. } => body.slice(0..5 * 1024).as_ref(), // first 5 KiB
        FetchResult::Error(e) => return RoutedOutcome::Failed(anyhow!(e)),
    };

    // Decide what we need to do with the response
    let outcome = decide_handling(&meta, dest, peek, policy);

    match (dest, outcome.decision) {
        (RequestDestination::MainDocument, HandlingDecision::Render { target }) => {
            /// We need to render it
            match target {
                RenderTarget::TextViewer => {
                    // We need to render it in the viewer (not through a parser)
                    RoutedOutcome::ViewerRendered
                }
                RenderTarget::HtmlParser => {
                    // Render through the HTML parser
                    if let Some(reader) = shared {
                        let doc = crate::engine::tab::worker::parse_main_document_stream(
                            req.tab_id, req.nav_id, meta.final_url.clone(), reader, req.cancel.clone(),
                            DummyHtml5Config::default(),
                            |evt| { let _ = event_tx.send(evt); },
                            |fetch_req| { let _ = io_tx.send(fetch_req); },
                        ).await.unwrap_or(Document("".to_string()));
                        NavigationResult::Document { meta, doc }
                    } else {
                        let bytes = body.as_ref().map(|b| b.as_ref()).unwrap_or(&peek);
                        let doc = crate::engine::tab::worker::parse_main_document_bytes(
                            req.tab_id, req.nav_id, meta.final_url.clone(), bytes,
                            ignore_cache, event_tx.clone()
                        ).await.unwrap_or(Document("".to_string()));
                        NavigationResult::Document { meta, doc }
                    }
                }
            }
        }
        (RequestDestination::MainDocument, HandlingDecision::Render(_)) => {
            // Render a nont textviewer or html parser
            hooks.viewer.render_top_level(outcome.class, meta, top).await?;
            RoutedOutcome::ViewerRendered
        }
        (RequestDestination::MainDocument, HandlingDecision::Download { path }) => {
            // Download resource if it's a main document
            let dest = hooks.download.resolve_or_prompt(path, &meta, &outcome).await?;
            hooks.download.to_file(top, &dest, &meta).await?;
            // You can split Started vs Finished if streaming.
            RoutedOutcome::DownloadFinished(dest)
        }
        (RequestDestination::MainDocument, HandlingDecision::Block(reason)) => RoutedOutcome::Blocked(reason),
        (RequestDestination::MainDocument, HandlingDecision::Cancel) => RoutedOutcome::Cancelled,
        (RequestDestination::MainDocument, HandlingDecision::OpenExternal) => {
            let p = hooks.external.stage_and_open(top, &meta).await?;
            RoutedOutcome::DownloadStarted(p)
        }

        // -------- Subresources (no UA prompts) --------
        (RequestDestination::Style, HandlingDecision::Render(RenderTarget::CssParser)) => {
            match fetch_result {
                FetchResult::Stream { shared, .. } => {
                    let id = hooks.css.load_stream(meta.final_url.clone(), shared).await?;
                    RoutedOutcome::CssLoaded(id)
                }
                FetchResult::Buffered { body, .. } => {
                    let id = hooks.css.load_bytes(meta.final_url.clone(), &body).await?;
                    RoutedOutcome::CssLoaded(id)
                }
                FetchResult::Error(_) => {
                    RoutedOutcome::Failed(anyhow!("Expected stream or buffered body for CSS"))
                }
            }
        }
        (RequestDestination::Script, HandlingDecision::Render(RenderTarget::JsEngine)) => {
            let id = hooks.js.exec(top, &meta).await?;
            RoutedOutcome::ScriptExecuted(id)
        }
        (RequestDestination::Image, HandlingDecision::Render(RenderTarget::ImageDecoder)) => {
            let id = hooks.images.decode(top, &meta).await?;
            RoutedOutcome::ImageDecoded(id)
        }
        (RequestDestination::Font, HandlingDecision::Render(RenderTarget::FontLoader)) => {
            let id = hooks.fonts.load(top, &meta).await?;
            RoutedOutcome::FontLoaded(id)
        }

        // Any other subresource decision that isn’t Render -> block (no download)
        (_, HandlingDecision::Block(reason)) => RoutedOutcome::Blocked(reason),
        (_, HandlingDecision::Cancel) => RoutedOutcome::Cancelled,
        // Safety net: e.g., Download/OpenExternal for subresources → treat as block
        (_, HandlingDecision::Download { .. } | HandlingDecision::OpenExternal | HandlingDecision::Render(_)) => RoutedOutcome::Blocked(BlockReason::Policy),
    }
}