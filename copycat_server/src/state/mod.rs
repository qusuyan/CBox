mod bitcoin;
pub use bitcoin::BitcoinState;
use copycat_protocol::ChainType;

pub enum ChainState {
    Dummy,
    Bitcoin { state: BitcoinState },
}

impl ChainState {
    pub fn new(chain_type: ChainType) -> Self {
        match chain_type {
            ChainType::Dummy => ChainState::Dummy,
            ChainType::Bitcoin => ChainState::Bitcoin {
                state: BitcoinState::new(),
            },
        }
    }
}
