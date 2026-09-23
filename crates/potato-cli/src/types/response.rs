use serde::{Deserialize, Serialize};
use super::action::Action;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    #[serde(rename = "SPECIFICATION")]
    Specification,
    #[serde(rename = "BOOTSTRAP")]
    Bootstrap,
    #[serde(rename = "IMPLEMENTATION")]
    Implementation,
    #[serde(rename = "VERIFY")]
    Verify,
    #[serde(rename = "REPAIR")]
    Repair,
    #[serde(rename = "COMPLETE")]
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurnResponse {
    pub thought: String,
    pub phase: Phase,
    pub action: Action,
}
