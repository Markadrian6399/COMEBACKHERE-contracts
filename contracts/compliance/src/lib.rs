#![no_std]

mod allowlist;
pub use allowlist::{AddressState, ComplianceError, DataKey};

use soroban_sdk::{contract, contracterror, contractimpl, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    Unauthorized = 1,
    ContractPaused = 2,
    AlreadyInitialized = 3,
}

#[contract]
pub struct ComplianceContract;

#[contractimpl]
impl ComplianceContract {
    /// Initialize the compliance contract with an admin address.
    ///
    /// # Parameters
    /// - `admin`: The initial administrator. Must authorize this call.
    ///
    /// # Errors
    /// - [`ContractError::AlreadyInitialized`] if the contract has already been initialized.
    ///
    /// # Events
    /// None emitted on initialization.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    /// Returns `true` if `address` is currently allowed (not blocked, not expired).
    ///
    /// # Parameters
    /// - `address`: The address to check.
    ///
    /// # Returns
    /// `true` when the address has been explicitly allowed, is not blocked, and any
    /// time-based allowance has not yet expired; `false` otherwise.
    pub fn is_allowed(env: Env, address: Address) -> bool {
        let blocked: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Blocked(address.clone()))
            .unwrap_or(false);
        if blocked {
            return false;
        }
        let allowed: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Allowed(address.clone()))
            .unwrap_or(false);
        if !allowed {
            return false;
        }
        // Check optional expiry
        if let Some(expires_at) = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::AllowedUntil(address))
        {
            return env.ledger().timestamp() < expires_at;
        }
        true
    }

    /// Permanently allow an address. Removes any existing expiry.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    /// - `address`: The address to allow.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    /// - [`ContractError::ContractPaused`] if the contract is paused.
    ///
    /// # Events
    /// Publishes `("address_allowed",) → address`.
    pub fn allow_address(env: Env, admin: Address, address: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        Self::require_not_paused(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::Allowed(address.clone()), &true);
        // Remove any expiry so this becomes a permanent allow.
        env.storage()
            .persistent()
            .remove(&DataKey::AllowedUntil(address.clone()));
        Self::track_address(&env, &address);
        env.events()
            .publish((Symbol::new(&env, "address_allowed"),), address);
        Ok(())
    }

    /// Block an address. Permitted even while the contract is paused (emergency policy)
    /// so the admin can remediate compromised addresses without unpausing first.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    /// - `address`: The address to block.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    ///
    /// # Events
    /// Publishes `("address_blocked",) → address`.
    pub fn block_address(env: Env, admin: Address, address: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::Blocked(address.clone()), &true);
        Self::track_address(&env, &address);
        env.events()
            .publish((Symbol::new(&env, "address_blocked"),), address);
        Ok(())
    }

    /// Allow an address until a specific ledger timestamp (seconds since epoch).
    ///
    /// After `expires_at`, [`is_allowed`](Self::is_allowed) returns `false` even if
    /// the `Allowed` flag is set.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    /// - `address`: The address to allow temporarily.
    /// - `expires_at`: Unix timestamp (seconds) after which the allowance expires.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    /// - [`ContractError::ContractPaused`] if the contract is paused.
    ///
    /// # Events
    /// Publishes `("address_allowed_until",) → (address, expires_at)`.
    pub fn allow_address_until(
        env: Env,
        admin: Address,
        address: Address,
        expires_at: u64,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        Self::require_not_paused(&env)?;
        env.storage()
            .persistent()
            .set(&DataKey::Allowed(address.clone()), &true);
        env.storage()
            .persistent()
            .set(&DataKey::AllowedUntil(address.clone()), &expires_at);
        Self::track_address(&env, &address);
        env.events().publish(
            (Symbol::new(&env, "address_allowed_until"),),
            (address, expires_at),
        );
        Ok(())
    }

    /// Initiate a two-step admin transfer. The pending admin must call
    /// [`accept_admin`](Self::accept_admin) to complete the handover.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    /// - `new_admin`: The address being nominated as the next administrator.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    ///
    /// # Events
    /// Publishes `("admin_transfer_initiated",) → new_admin`.
    pub fn transfer_admin(
        env: Env,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        env.events()
            .publish((Symbol::new(&env, "admin_transfer_initiated"),), new_admin);
        Ok(())
    }

    /// Complete the admin transfer initiated by [`transfer_admin`](Self::transfer_admin).
    ///
    /// Must be called by the pending admin to activate the new admin role.
    ///
    /// # Parameters
    /// - `new_admin`: The pending administrator. Must authorize this call.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `new_admin` does not match the stored pending admin.
    ///
    /// # Panics
    /// Panics with `"NoPendingAdmin"` if [`transfer_admin`](Self::transfer_admin) was never called.
    ///
    /// # Events
    /// Publishes `("admin_transferred",) → new_admin`.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), ContractError> {
        new_admin.require_auth();
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .expect("NoPendingAdmin");
        if pending != new_admin {
            return Err(ContractError::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events()
            .publish((Symbol::new(&env, "admin_transferred"),), new_admin);
        Ok(())
    }

    /// Remove the block flag and explicitly allow an address.
    ///
    /// Permitted even while paused (emergency policy). Does **not** remove an existing
    /// `AllowedUntil` expiry; call [`allow_address`](Self::allow_address) for a
    /// permanent, expiry-free allow.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    /// - `address`: The address to clear.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    ///
    /// # Events
    /// Publishes `("address_cleared",) → address`.
    pub fn clear_address(env: Env, admin: Address, address: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        env.storage()
            .persistent()
            .set(&DataKey::Blocked(address.clone()), &false);
        env.storage()
            .persistent()
            .set(&DataKey::Allowed(address.clone()), &true);
        Self::track_address(&env, &address);
        env.events()
            .publish((Symbol::new(&env, "address_cleared"),), address);
        Ok(())
    }

    /// Pause the contract. While paused, [`allow_address`](Self::allow_address) and
    /// [`allow_address_until`](Self::allow_address_until) are blocked.
    /// [`block_address`](Self::block_address) and [`clear_address`](Self::clear_address)
    /// remain available for emergency remediation.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    ///
    /// # Events
    /// Publishes `("compliance_paused",) → admin`.
    pub fn pause(env: Env, admin: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events()
            .publish((Symbol::new(&env, "compliance_paused"),), admin);
        Ok(())
    }

    /// Resume normal operation after a pause.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    ///
    /// # Errors
    /// - [`ContractError::Unauthorized`] if `admin` is not the stored administrator.
    ///
    /// # Events
    /// Publishes `("compliance_unpaused",) → admin`.
    pub fn unpause(env: Env, admin: Address) -> Result<(), ContractError> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((Symbol::new(&env, "compliance_unpaused"),), admin);
        Ok(())
    }

    /// Export a paginated snapshot of every tracked address and its current state.
    ///
    /// Requires admin authentication for audit-trail accountability. Use `start` and
    /// `limit` to page through large lists without exceeding Soroban's invocation budget.
    ///
    /// # Parameters
    /// - `admin`: Current administrator. Must authorize this call.
    /// - `start`: Zero-based index of the first entry to return.
    /// - `limit`: Maximum number of entries to return. Pass `0` for no cap.
    ///
    /// # Returns
    /// A [`Vec`] of `(address, state)` pairs, each reflecting the current
    /// [`AddressState`] of the tracked address.
    ///
    /// # Panics
    /// Panics with `"Unauthorized"` if `admin` is not the stored administrator.
    pub fn export_snapshot(
        env: Env,
        admin: Address,
        start: u32,
        limit: u32,
    ) -> Vec<(Address, AddressState)> {
        Self::require_admin(&env, &admin).expect("Unauthorized");
        let index: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AddressIndex)
            .unwrap_or(Vec::new(&env));
        let mut out: Vec<(Address, AddressState)> = Vec::new(&env);
        let total = index.len();
        let start = start.min(total);
        let end = if limit == 0 {
            total
        } else {
            (start + limit).min(total)
        };
        for i in start..end {
            let addr = index.get(i).unwrap();
            let state = Self::address_state(&env, &addr);
            out.push_back((addr, state));
        }
        out
    }

    /// Check the compliance state for a batch of addresses, with pagination.
    ///
    /// Unlike [`is_allowed`](Self::is_allowed), this returns the full [`AddressState`]
    /// for each address in `addresses`, skipping the first `start` entries and
    /// returning at most `limit` results. Does **not** require admin authentication.
    ///
    /// # Parameters
    /// - `addresses`: The list of addresses to check.
    /// - `start`: Zero-based index of the first address in `addresses` to evaluate.
    /// - `limit`: Maximum number of results to return. Pass `0` for no cap.
    ///
    /// # Returns
    /// A [`Vec`] of `(address, state)` pairs for the requested page.
    pub fn bulk_check_addresses(
        env: Env,
        addresses: Vec<Address>,
        start: u32,
        limit: u32,
    ) -> Vec<(Address, AddressState)> {
        let mut out: Vec<(Address, AddressState)> = Vec::new(&env);
        let total = addresses.len();
        let start = start.min(total);
        let end = if limit == 0 {
            total
        } else {
            (start + limit).min(total)
        };
        for i in start..end {
            let addr = addresses.get(i).unwrap();
            let state = Self::address_state(&env, &addr);
            out.push_back((addr, state));
        }
        out
    }

    fn require_admin(env: &Env, admin: &Address) -> Result<(), ContractError> {
        admin.require_auth();
        let stored: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if stored != *admin {
            return Err(ContractError::Unauthorized);
        }
        Ok(())
    }

    fn require_not_paused(env: &Env) -> Result<(), ContractError> {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(ContractError::ContractPaused);
        }
        Ok(())
    }

    /// Compute the current [`AddressState`] for a single address without auth.
    fn address_state(env: &Env, addr: &Address) -> AddressState {
        let blocked: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Blocked(addr.clone()))
            .unwrap_or(false);
        if blocked {
            return AddressState::Blocked;
        }
        let allowed: bool = env
            .storage()
            .persistent()
            .get(&DataKey::Allowed(addr.clone()))
            .unwrap_or(false);
        if !allowed {
            return AddressState::Blocked;
        }
        if let Some(expires_at) = env
            .storage()
            .persistent()
            .get::<_, u64>(&DataKey::AllowedUntil(addr.clone()))
        {
            if env.ledger().timestamp() < expires_at {
                AddressState::Allowed
            } else {
                AddressState::Expired
            }
        } else {
            AddressState::Allowed
        }
    }

    /// Adds `address` to the instance-level AddressIndex if not already present.
    fn track_address(env: &Env, address: &Address) {
        let mut index: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::AddressIndex)
            .unwrap_or(Vec::new(env));
        if !index.contains(address) {
            index.push_back(address.clone());
            env.storage()
                .instance()
                .set(&DataKey::AddressIndex, &index);
        }
    }
}

#[cfg(test)]
extern crate std;
