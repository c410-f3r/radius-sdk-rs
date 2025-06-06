use std::sync::Arc;

use serde::Serialize;

use crate::{
    address::Address, chain_type::ChainType, error::SignatureError, signature::Signature, traits::*,
};

pub struct PrivateKeySigner {
    inner: Arc<dyn Signer>,
}

unsafe impl Send for PrivateKeySigner {}

unsafe impl Sync for PrivateKeySigner {}

impl Clone for PrivateKeySigner {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> From<T> for PrivateKeySigner
where
    T: Signer + 'static,
{
    fn from(value: T) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }
}

impl PrivateKeySigner {
    pub fn from_slice(chain_type: ChainType, private_key: &[u8]) -> Result<Self, SignatureError> {
        chain_type.signer_builder().build_from_slice(private_key)
    }

    pub fn from_str(chain_type: ChainType, private_key: &str) -> Result<Self, SignatureError> {
        chain_type.signer_builder().build_from_str(private_key)
    }

    pub fn from_random(chain_type: ChainType) -> Result<(Self, String), SignatureError> {
        chain_type.signer_builder_random().build_from_random()
    }

    pub fn address(&self) -> &Address {
        self.inner.address()
    }

    pub fn sign_message<T>(&self, message: T) -> Result<Signature, SignatureError>
    where
        T: Serialize,
    {
        let message_bytes =
            bincode::serialize(&message).map_err(SignatureError::SerializeMessage)?;

        self.inner.sign_message(&message_bytes)
    }
}
