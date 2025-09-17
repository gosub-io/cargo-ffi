use std::sync::Arc;
use crate::net::{stream_to_bytes, SharedBody};
use crate::net::types::FetchResultMeta;

pub type DummyFont = String;

pub trait FontPipeline {
    async fn parse_stream(
        &self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: Arc<SharedBody>
    ) -> anyhow::Result<DummyFont>;

    async fn parse_bytes(
        &self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: &[u8],
    ) -> anyhow::Result<DummyFont>;
}


struct FontPipelineImpl;

impl FontPipeline for FontPipelineImpl {
    async fn parse_stream(&self, meta: FetchResultMeta, peek: &[u8], shared: Arc<SharedBody>) -> anyhow::Result<DummyFont> {
        // Normally, we send chunks to the Font parser. Right now, we just collect everything
        let b = stream_to_bytes(meta, peek.to_vec(), shared);
        Ok(String::from_utf8_lossy(b).to_string())
    }

    async fn parse_bytes(&self, meta: FetchResultMeta, peek: &[u8], body: &[u8]) -> anyhow::Result<DummyFont>{
        let mut bytes = Vec::with_capacity(peek.len() + body.len());
        bytes.extend_from_slice(peek);
        bytes.extend_from_slice(body);

        Ok(String::from_utf8_lossy(bytes.as_slice()).to_string())
    }
}