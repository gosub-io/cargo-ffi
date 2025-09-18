use std::sync::Arc;
use async_trait::async_trait;
use bytes::Buf;
use crate::html::DummyDocument;
use crate::net::{stream_to_bytes, SharedBody};
use crate::net::types::FetchResultMeta;

#[async_trait]
pub trait HtmlPipeline {
    async fn parse_stream(
        &mut self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: Arc<SharedBody>
    ) -> anyhow::Result<DummyDocument>;

    async fn parse_bytes(
        &mut self,
        meta: FetchResultMeta,
        body: &[u8],
    ) -> anyhow::Result<DummyDocument>;
}


struct HtmlPipelineImpl;

#[async_trait]
impl HtmlPipeline for HtmlPipelineImpl {
    async fn parse_stream(&mut self, meta: FetchResultMeta, peek: &[u8], shared: Arc<SharedBody>) -> anyhow::Result<DummyDocument> {
        // Normally, we send chunks to the Font parser. Right now, we just collect everything
        match stream_to_bytes(peek.to_vec(), shared).await {
            Ok(buf) => {
                let s = String::from_utf8_lossy(buf.chunk()).to_string();
                Ok(DummyDocument::from(s, meta.final_url.clone()))
            },
            Err(e) => Err(anyhow::anyhow!("Failed to read font stream: {}", e))
        }
    }

    async fn parse_bytes(&mut self, meta: FetchResultMeta, body: &[u8]) -> anyhow::Result<DummyDocument>{
        let s = String::from_utf8_lossy(body).to_string();
        Ok(DummyDocument::from(s, meta.final_url.clone()))
    }
}