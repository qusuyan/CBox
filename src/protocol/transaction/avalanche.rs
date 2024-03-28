use get_size::GetSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AvalancheTxn {}

impl GetSize for AvalancheTxn {}
