use crate::engine::pipeline::css::DummyStylesheet;
use crate::engine::pipeline::font::DummyFont;
use crate::engine::pipeline::js::DummyJsDocument;
use crate::engine::pipeline::Hooks;
use crate::engine::types::{IoChannel, PeekBuf, RequestId};
use crate::engine::UaPolicy;
use crate::html::{DummyDocument, ResourceHint};
use crate::net::decision::types::BlockReason;
use crate::net::types::{FetchHandle, FetchKeyData, FetchRequest, FetchResult, Initiator, ResourceKind};
use crate::net::{
    decide_handling, stream_to_bytes, submit_to_io, HandlingDecision, RenderTarget, RequestDestination, SharedBody,
};
use crate::zone::ZoneId;
use anyhow::anyhow;
use bytes::Bytes;
use http::Method;
use std::sync::Arc;

pub enum RoutedOutcome {
    MainDocument(DummyDocument),
    ViewerRendered(Bytes),
    DownloadStarted(std::path::PathBuf),
    DownloadFinished(std::path::PathBuf),

    CssLoaded(DummyStylesheet),
    ScriptExecuted(DummyJsDocument),
    ImageDecoded(image::DynamicImage),
    FontLoaded(DummyFont),

    Blocked(BlockReason),
    Cancelled,
}

/// Convert a RequestDestination to a ResourceKind
#[allow(unused)]
pub fn resource_kind_from_dest(dest: RequestDestination) -> ResourceKind {
    match dest {
        RequestDestination::MainDocument => ResourceKind::Document,
        RequestDestination::Style => ResourceKind::Stylesheet,
        RequestDestination::Script => ResourceKind::Script { blocking: false },
        RequestDestination::Image => ResourceKind::Image,
        RequestDestination::Font => ResourceKind::Font,
        RequestDestination::Other => ResourceKind::Other,
        RequestDestination::Audio => ResourceKind::Media,
        RequestDestination::Video => ResourceKind::Media,
        RequestDestination::Worker => ResourceKind::Other,
        RequestDestination::SharedWorker => ResourceKind::Other,
        RequestDestination::ServiceWorker => ResourceKind::Other,
        RequestDestination::Manifest => ResourceKind::Other,
        RequestDestination::Track => ResourceKind::Other,
        RequestDestination::Xslt => ResourceKind::Other,
        RequestDestination::Fetch => ResourceKind::Other,
        RequestDestination::Xhr => ResourceKind::Other,
    }
}

/// BodyContent represents either a streaming body or a fully buffered body.
enum BodyContent {
    Stream { shared: Arc<SharedBody> },
    Buffered { body: Bytes },
}

impl BodyContent {
    // Convert to bytses, collecting the stream if necessary. Will take the peek buffer into account (if needed)
    async fn to_bytes(self, peek_buf: PeekBuf) -> anyhow::Result<Bytes> {
        match self {
            BodyContent::Stream { shared } => {
                let buf = stream_to_bytes(peek_buf.clone(), shared).await?;
                Ok(Bytes::from(buf))
            }
            BodyContent::Buffered { body } => Ok(body),
        }
    }
}

/// Route a fetch result based on its destination and the UA policy.
pub async fn route_response_for(
    dest: RequestDestination,
    handle: FetchHandle,
    request: FetchRequest,
    fetch_result: FetchResult,
    policy: &UaPolicy,
    hooks: &mut Hooks,
) -> anyhow::Result<RoutedOutcome> {
    // Fetch the meta data, peek buffer and content (type)
    let (meta, body_content, peek_buf) = match fetch_result {
        FetchResult::Stream { meta, peek_buf, shared } => (meta, BodyContent::Stream { shared }, peek_buf),
        FetchResult::Buffered { meta, body } => {
            let peek_buf = PeekBuf::from_slice(&body[0..5 * 1024]);
            (meta, BodyContent::Buffered { body }, peek_buf)
        }
        FetchResult::Error(e) => {
            return Err(anyhow!(e));
        }
    };

    // Decide what we need to do with the response
    let outcome = decide_handling(&meta, dest, peek_buf.clone(), policy);

    match (dest, outcome.decision, body_content) {
        (RequestDestination::MainDocument, HandlingDecision::Render(target), body_content) => {
            // We need to render it
            match target {
                RenderTarget::TextViewer => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf.clone()).await?,
                )),
                RenderTarget::HtmlParser => {
                    let doc = match body_content {
                        BodyContent::Stream { shared } => {
                            hooks
                                .html
                                .parse_stream(request, handle, meta, peek_buf, shared)
                                .await?
                        }
                        BodyContent::Buffered { body } => {
                            hooks
                                .html
                                .parse_bytes(request, handle, meta, body.as_ref())
                                .await?
                        }
                    };
                    Ok(RoutedOutcome::MainDocument(doc))
                    // Render through the HTML parser
                    // if let Some(reader) = shared {
                    //     let doc = crate::engine::tab::worker::parse_main_document_stream(
                    //         req.tab_id, req.nav_id, meta.final_url.clone(), reader, req.cancel.clone(),
                    //         DummyHtml5Config::default(),
                    //         |evt| { let _ = event_tx.send(evt); },
                    //         |fetch_req| { let _ = io_tx.send(fetch_req); },
                    //     ).await.unwrap_or(Document("".to_string()));
                    //     NavigationResult::Document { meta, doc }
                    // } else {
                    //     let bytes = body.as_ref().map(|b| b.as_ref()).unwrap_or(&peek);
                    //     let doc = crate::engine::tab::worker::parse_main_document_bytes(
                    //         req.tab_id, req.nav_id, meta.final_url.clone(), bytes,
                    //         ignore_cache, event_tx.clone()
                    //     ).await.unwrap_or(Document("".to_string()));
                    //     NavigationResult::Document { meta, doc }
                    // }
                }
                RenderTarget::CssParser => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
                RenderTarget::JsEngine => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
                RenderTarget::ImageDecoder => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
                RenderTarget::MediaPipeline => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
                RenderTarget::FontLoader => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
                RenderTarget::PdfViewer => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
                RenderTarget::BodyToJs => Ok(RoutedOutcome::ViewerRendered(
                    body_content.to_bytes(peek_buf).await?,
                )),
            }
        }
        // (RequestDestination::MainDocument, HandlingDecision::Render(_)) => {
        //     // Render a non text viewer or html parser
        //     // hooks.viewer.render_top_level(outcome.class, meta, top).await?;
        //     Ok(RoutedOutcome::ViewerRendered(fetch_result.to_bytes().await?))
        // }
        (RequestDestination::MainDocument, HandlingDecision::Download { .. }, _) => {
            // Download resource if it's a main document
            // let dest = hooks.download.resolve_or_prompt(path, &meta, &outcome).await?;
            // hooks.download.to_file(top, &dest, &meta).await?;
            // You can split Started vs Finished if streaming.
            // RoutedOutcome::DownloadFinished(dest)
            Err(anyhow!("Cannot download main document"))
        }
        (RequestDestination::MainDocument, HandlingDecision::Block(reason), ..) => Ok(RoutedOutcome::Blocked(reason)),
        (RequestDestination::MainDocument, HandlingDecision::Cancel, ..) => Ok(RoutedOutcome::Cancelled),
        (RequestDestination::MainDocument, HandlingDecision::OpenExternal, ..) => {
            // let p = hooks.external.stage_and_open(top, &meta).await?;
            // RoutedOutcome::DownloadStarted(p)
            Err(anyhow!("Cannot open main document in external application"))
        }

        // -------- Sub resources (no UA prompts) --------
        (RequestDestination::Style, HandlingDecision::Render(RenderTarget::CssParser), body_content) => {
            let stylesheet = match body_content {
                BodyContent::Stream { shared } => hooks.css.parse_stream(meta, peek_buf, shared).await?,
                BodyContent::Buffered { body } => hooks.css.parse_bytes(meta, body.as_ref()).await?,
            };
            Ok(RoutedOutcome::CssLoaded(stylesheet))
        }
        (RequestDestination::Script, HandlingDecision::Render(RenderTarget::JsEngine), body_content) => {
            let script = match body_content {
                BodyContent::Stream { shared } => hooks.js.parse_stream(meta, peek_buf, shared).await?,
                BodyContent::Buffered { body } => hooks.js.parse_bytes(meta, body.as_ref()).await?,
            };
            Ok(RoutedOutcome::ScriptExecuted(script))
        }
        (RequestDestination::Image, HandlingDecision::Render(RenderTarget::ImageDecoder), body_content) => {
            let image = match body_content {
                BodyContent::Stream { shared } => hooks.images.parse_stream(meta, peek_buf, shared).await?,
                BodyContent::Buffered { body } => hooks.images.parse_bytes(meta, body.as_ref()).await?,
            };
            Ok(RoutedOutcome::ImageDecoded(image))
        }
        (RequestDestination::Font, HandlingDecision::Render(RenderTarget::FontLoader), body_content) => {
            let font = match body_content {
                BodyContent::Stream { shared } => hooks.fonts.parse_stream(meta, peek_buf, shared).await?,
                BodyContent::Buffered { body } => hooks.fonts.parse_bytes(meta, body.as_ref()).await?,
            };
            Ok(RoutedOutcome::FontLoaded(font))
        }

        // Any other subresource decision that isn’t Render -> block (no download)
        (_, HandlingDecision::Block(reason), _) => Ok(RoutedOutcome::Blocked(reason)),
        (_, HandlingDecision::Cancel, _) => Ok(RoutedOutcome::Cancelled),

        // Safety net: e.g., Download/OpenExternal for sub resources: treat as block
        (_, HandlingDecision::Download { .. } | HandlingDecision::OpenExternal | HandlingDecision::Render(_), _) => {
            Ok(RoutedOutcome::Blocked(BlockReason::Policy))
        }
    }
}

/// Fetch a subresource and route it based on its destination and the UA policy.
#[allow(unused)]
pub async fn fetch_and_route_subresource(
    zone_id: ZoneId,
    parent_handle: &FetchHandle,
    parent_request: &FetchRequest,
    hint: ResourceHint,
    io_tx: IoChannel,
    policy: &UaPolicy,
    hooks: &mut Hooks,
) -> anyhow::Result<RoutedOutcome> {
    let sub_req = FetchRequest {
        req_id: RequestId::new(),
        reference: parent_request.reference,
        key_data: FetchKeyData {
            url: hint.url,
            method: Method::GET,
            headers: Default::default(),
        },
        priority: hint.priority,
        kind: resource_kind_from_dest(hint.dest),
        initiator: Initiator::Parser,
        streaming: true,
        auto_decode: true,
        max_bytes: None,
    };

    let (handle, rx) = submit_to_io(zone_id, sub_req, io_tx, Some(parent_handle.cancel.clone()))
        .await
        .map_err(|e| anyhow!("Failed to submit fetch to IO thread: {}", e))?;

    let fetch_result: FetchResult = rx
        .await
        .map_err(|e| anyhow!("Failed to receive fetch result: {}", e))?;

    route_response_for(
        hint.dest,
        handle,
        parent_request.clone(),
        fetch_result,
        policy,
        hooks,
    )
    .await
}
