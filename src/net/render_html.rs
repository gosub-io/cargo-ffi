use tokio::io::AsyncRead;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::engine::types::RequestId;
use crate::events::{EngineEvent, LoadEvent, NavigationEvent, ResourceEvent};
use crate::html::{DummyDocument, DummyHtml5Config, ResourceHint};
use crate::NavigationId;
use crate::net::NavigationError;
use crate::net::types::{FetchRequest, Initiator, Priority};
use crate::tab::TabId;

pub async fn render_main_html<R>(
    tab_id: TabId,
    nav_id: NavigationId,
    final_url: Url,
    reader: R,
    cancel: CancellationToken,
    cfg: DummyHtml5Config,
    mut emit_event: impl FnMut(EngineEvent) + Send,
    mut enqueue_fetch: impl FnMut(FetchRequest) + Send,
) -> Result<DummyDocument, NavigationError>
where
    R: AsyncRead + Unpin + Send + 'static
{
    let mut on_discover = |hint: ResourceHint| {
        let req = FetchRequest {
            tab_id,
            nav_id,
            req_id: RequestId::new(),
            kind: hint.kind,
            streaming: false,
            auto_decode: false,
            max_bytes: None,
            cancel,
            priority: hint.priority,
            referrer: Some(final_url.clone()),
            key_data: FetchKeyData {},
            initiator: Initiator::Navigation,
            reply: None,
        };
        enqueue_fetch(req);

        emit_event(EngineEvent::Resource {
            tab_id,
            event: ResourceEvent::Queued {
                nav_id,
                kind: hint.kind,
                initiator: Initiator::Parser,
                url: hint.url,
                priority: Priority::High,
            },
        })
    };

    let doc = parse_main_document_stream(
        final_url.clone(),
        reader,
        cancel,
        cfg,
        &mut on_discover,
    ).await.map_err(|e| NavigationError::Other(e) )?;

    // Fire whatever document/ready events you expose
    emit_event(EngineEvent::Load { tab_id, event: LoadEvent::MainDocumentParsed { nav_id, title: doc.title.clone() }});
    emit_event(EngineEvent::Navigation { tab_id, event: NavigationEvent::Committed { nav_id, url: final_url }});

    Ok(doc)
}
