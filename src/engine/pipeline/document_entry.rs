pub fn start_document_pipeline(
    meta: ResponseMeta,
    stream: BodyStream,
    cancel: CancellationToken,
    event_tx: EventChannel,
) {
    tokio::spawn(async move {
        // let res = parse_main_document_stream(
        //     tab_id, meta.nav_id, meta.final_url.clone(),
        //     stream, cancel.clone(), /* ignore_cache */ false, event_tx.clone()
        // ).await;

        let res = Result<(), String> = Ok(());

        match res {
            Ok(()) => {
                let _ = event_tx.send(EngineEvent::NavigationEvent {
                    tab_id: meta.tab_id,
                    nav_id: meta.nav_id,
                    event: NetEvent::Finished {
                        result: Ok(NavigationOutput {
                            meta,
                            resource: Resource {
                                kind: ResourceKind::Document,
                                url: meta.final_url,
                                mime_type: "text/html".into(),
                                size: 0, // Placeholder
                                last_modified: None,
                                headers: HeaderMap::new(),
                            },
                        }),
                    },
                }).await;
            }
            Err(e) => {
                let _ = event_tx.send(EngineEvent::NavigationEvent {
                    tab_id: meta.tab_id,
                    nav_id: meta.nav_id,
                    event: NetEvent::Failed {
                        url: meta.final_url,
                        error: e,
                    },
                }).await;
            }
        }
    })
}