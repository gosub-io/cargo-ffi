use std::future::Future;
use std::sync::Arc;
use crate::net::{stream_to_bytes, SharedBody};
use crate::net::types::FetchResultMeta;

pub type DummyStylesheet = String;

pub trait CssPipeline {
    async fn parse_stream(
        &self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: Arc<SharedBody>
    ) -> anyhow::Result<DummyStylesheet>;

    async fn parse_bytes(
        &self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: &[u8],
    ) -> anyhow::Result<DummyStylesheet>;
}


struct CssPipelineImpl;

impl CssPipeline for CssPipelineImpl {
    async fn parse_stream(&self, meta: FetchResultMeta, peek: &[u8], shared: Arc<SharedBody>) -> anyhow::Result<DummyStylesheet> {
        // Normally, we send chunks to the CSS parser. Right now, we just collect everything
        let b = stream_to_bytes(meta, peek.to_vec(), shared);
        Ok(String::from_utf8_lossy(b).to_string())
    }

    async fn parse_bytes(&self, meta: FetchResultMeta, peek: &[u8], body: &[u8]) -> anyhow::Result<DummyStylesheet>{
        let mut bytes = Vec::with_capacity(peek.len() + body.len());
        bytes.extend_from_slice(peek);
        bytes.extend_from_slice(body);

        Ok(String::from_utf8_lossy(bytes.as_slice()).to_string())
    }
}