use std::sync::Arc;
use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use tokio_util::io::StreamReader;
use crate::engine::types::{IoChannel, PeekBuf, RequestId};
use crate::events::IoCommand;
use crate::html::{parse_main_document_stream, DummyDocument, ResourceHint};
use crate::net::SharedBody;
use crate::net::types::{FetchHandle, FetchRequest, FetchResultMeta, Initiator};

#[async_trait]
pub trait HtmlPipeline {
    async fn parse_stream(
        &mut self,
        request: FetchRequest,
        meta: FetchResultMeta,
        peek_buf: PeekBuf,
        body: Arc<SharedBody>
    ) -> anyhow::Result<DummyDocument>;

    async fn parse_bytes(
        &mut self,
        request: FetchRequest,
        meta: FetchResultMeta,
        body: &[u8],
    ) -> anyhow::Result<DummyDocument>;
}


pub struct HtmlPipelineImpl {
    io_tx: IoChannel,
}

impl HtmlPipelineImpl {
    pub fn new(io_tx: IoChannel) -> Self {
        Self { io_tx }
    }
}

#[async_trait]
impl HtmlPipeline for HtmlPipelineImpl {
    async fn parse_stream(
        &mut self,
        request: FetchRequest,
        meta: FetchResultMeta,
        peek_buf: PeekBuf,
        shared: Arc<SharedBody>,
    ) -> anyhow::Result<DummyDocument> {

        let cfg = crate::html::DummyHtml5Config::default();
        let on_discover = |hint: ResourceHint| {
            // For now, we do nothing when discovering resources
            println!("Discovered a resource hint: {:?}", hint);

            // Create a request for the discovered resource
            let request = FetchRequest {
                reference: request.reference.clone(),
                req_id: RequestId::new(),
                key_data: request.key_data.clone(),
                priority: hint.priority,
                initiator: Initiator::Parser,
                kind: hint.kind,
                streaming: true,
                auto_decode: false,
                max_bytes: None,
            };

            self.io_tx.send(IoCommand::Fetch {
                zone_id: fetch_handle.zone_id,
                req: request,
                handle: fetch_handle.handle,
                reply_tx: fetch_handle.reply_tx }).unwrap();
        };

        let res = parse_main_document_stream(
            meta.final_url, // This is the base URL
            SharedBody::combined_reader(peek_buf, shared),
            request.cancel.clone(),
            cfg,
            on_discover
        ).await;

        match res {
            Ok(doc) => Ok(doc),
            Err(e) => Err(anyhow!("Failed to parse HTML document: {:?}", e)),
        }
    }

    async fn parse_bytes(&mut self, request: FetchRequest, meta: FetchResultMeta, body: &[u8]) -> anyhow::Result<DummyDocument>{
        let cfg = crate::html::DummyHtml5Config::default();
        let on_discover = |hint: ResourceHint| {
            println!("Discovered a resource hint: {:?}", hint);

            // Create a request
        };

        let stream = stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(body))]);

        let res = parse_main_document_stream(
            meta.final_url, // This is the base URL
            StreamReader::new(stream),
            request.cancel.clone(),
            cfg,
            on_discover
        ).await;

        match res {
            Ok(doc) => Ok(doc),
            Err(e) => Err(anyhow!("Failed to parse HTML document: {:?}", e)),
        }
    }
}