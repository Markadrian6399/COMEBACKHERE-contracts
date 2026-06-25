use crate::multisig::DataKey;
use soroban_sdk::{Address, Env};

/// Returns the approval weight assigned to `signer`, or `0` if not registered.
pub fn signer_weight(env: &Env, signer: &Address) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::Signer(signer.clone()))
        .unwrap_or(0)
}

/// Requires `signer` to authenticate and have a non-zero weight in the signer registry.
/// Panics: `UnauthorizedSigner`.
pub fn require_authorized_signer(env: &Env, signer: &Address) {
    signer.require_auth();
    if signer_weight(env, signer) == 0 {
        panic!("UnauthorizedSigner");
    }
}
