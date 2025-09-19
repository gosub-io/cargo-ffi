use std::io::Cursor;
use std::sync::Arc;
use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use tokio_util::io::StreamReader;
use tokio_util::sync::CancellationToken;
use crate::engine::types::PeekBuf;
use crate::html::{parse_main_document_stream, DummyDocument, ResourceHint};
use crate::net::SharedBody;
use crate::net::types::FetchResultMeta;

#[async_trait]
pub trait HtmlPipeline {
    async fn parse_stream(
        &mut self,
        cancel_token: CancellationToken,
        meta: FetchResultMeta,
        peek_buf: PeekBuf,
        body: Arc<SharedBody>
    ) -> anyhow::Result<DummyDocument>;

    async fn parse_bytes(
        &mut self,
        cancel_token: CancellationToken,
        meta: FetchResultMeta,
        body: &[u8],
    ) -> anyhow::Result<DummyDocument>;
}


pub struct HtmlPipelineImpl;

#[async_trait]
impl HtmlPipeline for HtmlPipelineImpl {
    async fn parse_stream(
        &mut self,
        cancel_token: CancellationToken,
        meta: FetchResultMeta,
        peek_buf: PeekBuf,
        shared: Arc<SharedBody>
    ) -> anyhow::Result<DummyDocument> {

        let cfg = crate::html::DummyHtml5Config::default();
        let on_discover = |hint: ResourceHint| {
            // For now, we do nothing when discovering resources
            println!("Discovered a resource hint: {:?}", hint);
        };

        let res = parse_main_document_stream(
            meta.final_url, // This is the base URL
            SharedBody::combined_reader(peek_buf, shared),
            cancel_token,
            cfg,
            on_discover
        ).await;

        match res {
            Ok(doc) => Ok(doc),
            Err(e) => Err(anyhow!("Failed to parse HTML document: {:?}", e)),
        }
    }

    async fn parse_bytes(&mut self, cancel_token: CancellationToken, meta: FetchResultMeta, body: &[u8]) -> anyhow::Result<DummyDocument>{
        let cfg = crate::html::DummyHtml5Config::default();
        let on_discover = |hint: ResourceHint| {
            println!("Discovered a resource hint: {:?}", hint);

            // Create a request
        };

        let stream = stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(body))]);

        let res = parse_main_document_stream(
            meta.final_url, // This is the base URL
            StreamReader::new(stream),
            cancel_token,
            cfg,
            on_discover
        ).await;

        match res {
            Ok(doc) => Ok(doc),
            Err(e) => Err(anyhow!("Failed to parse HTML document: {:?}", e)),
        }
    }
}