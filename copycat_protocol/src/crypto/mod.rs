mod dummy;

use copycat_utils::CopycatError;

pub type Hash = Vec<u8>;
pub type PrivKey = Vec<u8>;
pub type PubKey = Vec<u8>;
pub type Signature = Vec<u8>;

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum CryptoScheme {
    Dummy,
}

impl CryptoScheme {
    pub fn sign(&self, privkey: &PrivKey, input: &[u8]) -> Result<Signature, CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::sign(privkey, input),
        }
    }

    pub fn verify(
        &self,
        pubkey: &PubKey,
        input: &[u8],
        signature: &Signature,
    ) -> Result<bool, CopycatError> {
        match self {
            CryptoScheme::Dummy => dummy::verify(pubkey, input, signature),
        }
    }
}
