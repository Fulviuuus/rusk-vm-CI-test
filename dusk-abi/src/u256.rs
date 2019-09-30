use serde::{Deserialize, Deserializer, Serialize, Serializer};

use ethereum_types::U256 as Wrapped;

/// Newtype for ethereum_types::U256, to support serde without std
pub struct U256(Wrapped);

impl core::fmt::Debug for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        //
        Ok(())
    }
}

impl Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_unit()
    }
}

impl<'de> Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Dummy for now
        Ok(U256(Wrapped::zero()))
    }
}

impl U256 {
    pub fn zero() -> Self {
        U256(Wrapped::zero())
    }
}
