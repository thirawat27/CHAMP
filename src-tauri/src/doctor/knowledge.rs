//! The bundled knowledge base of known failures.

use super::KnowledgeBase;
use std::sync::OnceLock;

pub(super) const KNOWLEDGE_JSON: &str = include_str!("../../doctor-knowledge.json");

pub(super) fn knowledge() -> &'static KnowledgeBase {
    static KNOWLEDGE: OnceLock<KnowledgeBase> = OnceLock::new();
    KNOWLEDGE.get_or_init(|| {
        serde_json::from_str(KNOWLEDGE_JSON).expect("embedded doctor-knowledge.json must be valid")
    })
}
