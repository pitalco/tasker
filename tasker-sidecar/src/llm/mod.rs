pub mod client;
pub mod prompts;
// Railway client kept for future use (paid model support)
#[allow(dead_code)]
mod railway_client;

pub use client::{LLMClient, LLMConfig, LLMProvider};
