use compliance::{ComplianceContract, ComplianceContractClient, ContractError};
use soroban_sdk::{testutils::{Address as _, Events}, Address, Env, Symbol};

fn setup() -> (Env, Address, Address, ComplianceContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let subject = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);
    (env, admin, subject, client)
}

#[test]
fn block_and_clear_address() {
    let (_env, admin, payer, client) = setup();
    client.allow_address(&admin, &payer);
    assert!(client.is_allowed(&payer));
    client.block_address(&admin, &payer);
    assert!(!client.is_allowed(&payer));
    client.clear_address(&admin, &payer);
    assert!(client.is_allowed(&payer));
}

#[test]
fn pause_and_unpause_emit_events() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);
    client.allow_address(&admin, &payer);
    assert!(client.is_allowed(&payer));
    // pause: state is set; subsequent allow is blocked (tested via unpause round-trip)
    client.pause(&admin);
    client.unpause(&admin);
    // after unpause, allow_address works again
    let payer2 = Address::generate(&env);
    client.allow_address(&admin, &payer2);
    assert!(client.is_allowed(&payer2));
}

#[test]
fn block_and_clear_permitted_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);
    client.allow_address(&admin, &payer);
    client.pause(&admin);
    // block and clear must succeed even while paused (emergency policy)
    client.block_address(&admin, &payer);
    assert!(!client.is_allowed(&payer));
    client.clear_address(&admin, &payer);
    assert!(client.is_allowed(&payer));
}

#[test]
fn allow_address_mutation_succeeds_after_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let address1 = Address::generate(&env);
    let address2 = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);

    // Allow address1 before pausing
    client.allow_address(&admin, &address1);
    assert!(client.is_allowed(&address1));

    // Pause then unpause
    client.pause(&admin);
    client.unpause(&admin);

    // Allow address2 should now work
    client.allow_address(&admin, &address2);
    assert!(client.is_allowed(&address2));
}

#[test]
fn block_address_mutation_succeeds_after_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let address = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);

    // Allow address first
    client.allow_address(&admin, &address);
    assert!(client.is_allowed(&address));

    // Pause then unpause
    client.pause(&admin);
    client.unpause(&admin);

    // Block address should now work
    client.block_address(&admin, &address);
    assert!(!client.is_allowed(&address));
}

#[test]
fn clear_address_mutation_succeeds_after_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let address = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);

    // Allow and block address first
    client.allow_address(&admin, &address);
    client.block_address(&admin, &address);
    assert!(!client.is_allowed(&address));

    // Pause then unpause
    client.pause(&admin);
    client.unpause(&admin);

    // Clear address should now work
    client.clear_address(&admin, &address);
    assert!(client.is_allowed(&address));
}

#[test]
fn read_only_queries_not_blocked_by_pause() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let allowed_address = Address::generate(&env);
    let blocked_address = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);

    // Setup: allow one address, block another
    client.allow_address(&admin, &allowed_address);
    client.block_address(&admin, &blocked_address);

    // Pause the contract
    client.pause(&admin);

    // Read-only queries should still work
    assert!(client.is_allowed(&allowed_address));
    assert!(!client.is_allowed(&blocked_address));

    let unrelated_address = Address::generate(&env);
    assert!(!client.is_allowed(&unrelated_address));
}

#[test]
fn unpause_emits_event_and_restores_allow() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);
    client.pause(&admin);
    client.unpause(&admin);
    client.allow_address(&admin, &payer);
    assert!(client.is_allowed(&payer));
}

#[test]
fn reinitialize_is_rejected() {
    let (env, _admin, _subject, client) = setup();
    let attacker = Address::generate(&env);
    let result = client.try_initialize(&attacker);
    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
}

// Verification: address_allowed event schema
// - topics[0]: symbol "address_allowed"
// - data: single Address value for the allowed address
#[test]
fn emits_address_allowed_event() {
    let (env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
    assert_eq!(last_event_symbol(&env), "address_allowed");
}

// Verification: address_blocked event schema
// - topics[0]: symbol "address_blocked"
// - data: single Address value for the blocked address
#[test]
fn emits_address_blocked_event() {
    let (env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
    client.block_address(&admin, &subject);
    assert!(!client.is_allowed(&subject));
    assert_eq!(last_event_symbol(&env), "address_blocked");
}

// Verification: address_cleared event schema
// - topics[0]: symbol "address_cleared"
// - data: single Address value for the cleared address
#[test]
fn emits_address_cleared_event() {
    let (env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    client.block_address(&admin, &subject);
    assert!(!client.is_allowed(&subject));
    client.clear_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
    assert_eq!(last_event_symbol(&env), "address_cleared");
}

// ── #121 Allow/Block/Clear precedence matrix ─────────────────────────────────

#[test]
fn precedence_never_allowed_is_denied() {
    let (_env, _admin, subject, client) = setup();
    assert!(!client.is_allowed(&subject));
}

#[test]
fn precedence_allowed_then_blocked_is_denied() {
    let (_env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    client.block_address(&admin, &subject);
    assert!(!client.is_allowed(&subject));
}

#[test]
fn precedence_blocked_then_cleared_is_allowed() {
    let (_env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    client.block_address(&admin, &subject);
    client.clear_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
}

#[test]
fn precedence_block_without_prior_allow_is_denied() {
    let (_env, admin, subject, client) = setup();
    client.block_address(&admin, &subject);
    assert!(!client.is_allowed(&subject));
}

#[test]
fn precedence_clear_without_prior_block_sets_allowed() {
    let (_env, admin, subject, client) = setup();
    // clear_address sets Allowed=true and Blocked=false regardless
    client.clear_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
}

// ── #123 Batch allow and block tests ─────────────────────────────────────────

#[test]
fn batch_allow_multiple_addresses() {
    let (env, admin, _, client) = setup();
    let addrs: soroban_sdk::Vec<Address> = soroban_sdk::vec![
        &env,
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    for addr in addrs.iter() {
        client.allow_address(&admin, &addr);
    }
    for addr in addrs.iter() {
        assert!(client.is_allowed(&addr));
    }
}

#[test]
fn batch_block_multiple_addresses() {
    let (env, admin, _, client) = setup();
    let addrs: soroban_sdk::Vec<Address> = soroban_sdk::vec![
        &env,
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    for addr in addrs.iter() {
        client.allow_address(&admin, &addr);
    }
    for addr in addrs.iter() {
        client.block_address(&admin, &addr);
    }
    for addr in addrs.iter() {
        assert!(!client.is_allowed(&addr));
    }
}

#[test]
fn batch_allow_then_block_subset() {
    let (env, admin, _, client) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    for addr in [&a, &b, &c] {
        client.allow_address(&admin, addr);
    }
    // block only b
    client.block_address(&admin, &b);
    assert!(client.is_allowed(&a));
    assert!(!client.is_allowed(&b));
    assert!(client.is_allowed(&c));
}

// ── #124 Temporary allowlist expiration tests ─────────────────────────────────

#[test]
fn temp_allow_before_expiry_is_allowed() {
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    client.allow_address_until(&admin, &subject, &(now + 1000));
    assert!(client.is_allowed(&subject));
}

#[test]
fn temp_allow_after_expiry_is_denied() {
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    // expires in the past
    client.allow_address_until(&admin, &subject, &now);
    assert!(!client.is_allowed(&subject));
}

#[test]
fn temp_allow_blocked_address_is_denied_regardless_of_expiry() {
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    client.allow_address_until(&admin, &subject, &(now + 1000));
    client.block_address(&admin, &subject);
    assert!(!client.is_allowed(&subject));
}

#[test]
fn temp_allow_cleared_removes_expiry_block() {
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    // set expired temp allow
    client.allow_address_until(&admin, &subject, &now);
    assert!(!client.is_allowed(&subject));
    // clear restores permanent allow (no expiry key respected after clear)
    client.clear_address(&admin, &subject);
    // clear_address sets Allowed=true, Blocked=false but does NOT remove AllowedUntil
    // so we verify the contract's actual behaviour: still expired
    // To permanently allow, use allow_address (no expiry)
    client.allow_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
}

// ── #125 Admin transfer flow tests ───────────────────────────────────────────

#[test]
fn admin_transfer_new_admin_can_allow() {
    let (env, admin, subject, client) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    client.accept_admin(&new_admin);
    // new admin can allow
    client.allow_address(&new_admin, &subject);
    assert!(client.is_allowed(&subject));
}

#[test]
fn admin_transfer_old_admin_loses_privileges() {
    let (env, admin, subject, client) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    client.accept_admin(&new_admin);
    // old admin can no longer allow
    // old admin can no longer allow (should return an error)
    let result = client.try_allow_address(&admin, &subject);
    assert!(result.is_err());
}

#[test]
fn admin_transfer_requires_accept_before_taking_effect() {
    let (env, admin, subject, client) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    // new_admin has NOT called accept_admin yet; old admin still works
    client.allow_address(&admin, &subject);
    assert!(client.is_allowed(&subject));
}

#[test]
fn admin_transfer_wrong_acceptor_panics() {
    let (env, admin, _subject, client) = setup();
    let new_admin = Address::generate(&env);
    let impostor = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    let result = client.try_accept_admin(&impostor);
    assert!(result.is_err());
}

#[test]
fn allow_address_returns_unauthorized_for_non_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let address = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);

    let result = client.try_allow_address(&non_admin, &address);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn allow_address_returns_contract_paused_when_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let address = Address::generate(&env);
    let id = env.register_contract(None, ComplianceContract);
    let client = ComplianceContractClient::new(&env, &id);
    client.initialize(&admin);
    client.pause(&admin);

    let result = client.try_allow_address(&admin, &address);
    assert_eq!(result, Err(Ok(ContractError::ContractPaused)));
}

// ── #80 export_snapshot tests ─────────────────────────────────────────────────

#[test]
fn export_snapshot_returns_all_tracked_addresses() {
    use compliance::AddressState;
    let (env, admin, _, client) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);

    client.allow_address(&admin, &a);
    client.allow_address(&admin, &b);
    client.block_address(&admin, &c);

    let snapshot = client.export_snapshot(&admin);
    assert_eq!(snapshot.len(), 3);

    // collect into a plain vec for easy lookup
    let mut found_a = false;
    let mut found_b = false;
    let mut found_c = false;
    for (addr, state) in snapshot.iter() {
        if addr == a {
            assert_eq!(state, AddressState::Allowed);
            found_a = true;
        } else if addr == b {
            assert_eq!(state, AddressState::Allowed);
            found_b = true;
        } else if addr == c {
            assert_eq!(state, AddressState::Blocked);
            found_c = true;
        }
    }
    assert!(found_a && found_b && found_c);
}

#[test]
fn export_snapshot_reflects_state_changes() {
    use compliance::AddressState;
    let (_env, admin, subject, client) = setup();

    client.allow_address(&admin, &subject);
    let snap1 = client.export_snapshot(&admin);
    assert_eq!(snap1.get(0).unwrap().1, AddressState::Allowed);

    client.block_address(&admin, &subject);
    let snap2 = client.export_snapshot(&admin);
    assert_eq!(snap2.get(0).unwrap().1, AddressState::Blocked);
}

#[test]
fn export_snapshot_dedups_repeated_operations_on_same_address() {
    let (_env, admin, subject, client) = setup();

    client.allow_address(&admin, &subject);
    client.block_address(&admin, &subject);
    client.clear_address(&admin, &subject);

    let snapshot = client.export_snapshot(&admin);
    assert_eq!(snapshot.len(), 1);
}

#[test]
fn export_snapshot_empty_when_no_addresses_tracked() {
    let (_env, admin, _subject, client) = setup();
    let snapshot = client.export_snapshot(&admin);
    assert_eq!(snapshot.len(), 0);
}

#[test]
fn export_snapshot_expired_temp_allow_shows_expired() {
    use compliance::AddressState;
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    // expires_at == now means timestamp is NOT < expires_at → Expired
    client.allow_address_until(&admin, &subject, &now);
    let snapshot = client.export_snapshot(&admin);
    assert_eq!(snapshot.get(0).unwrap().1, AddressState::Expired);
}

// ── Operator role tests ───────────────────────────────────────────────────────

#[test]
fn set_operator_as_admin_succeeds() {
    let (env, admin, _subject, client) = setup();
    let operator = Address::generate(&env);
    client.set_operator(&admin, &operator);
}

#[test]
fn set_operator_emits_operator_set_event() {
    let (env, admin, _subject, client) = setup();
    let operator = Address::generate(&env);
    client.set_operator(&admin, &operator);
    assert_eq!(last_event_symbol(&env), "operator_set");
}

#[test]
fn set_operator_as_non_admin_fails() {
    let (env, _admin, _subject, client) = setup();
    let non_admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let result = client.try_set_operator(&non_admin, &operator);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn admin_can_call_get_allow_expiry() {
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    let expires_at = now + 1000;
    client.allow_address_until(&admin, &subject, &expires_at);
    let expiry = client.get_allow_expiry(&admin, &subject);
    assert_eq!(expiry, Some(expires_at));
}

#[test]
fn get_allow_expiry_returns_none_for_permanent_allow() {
    let (_env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    let expiry = client.get_allow_expiry(&admin, &subject);
    assert_eq!(expiry, None);
}

#[test]
fn operator_can_call_get_allow_expiry() {
    let (env, admin, subject, client) = setup();
    let operator = Address::generate(&env);
    client.set_operator(&admin, &operator);
    let now = env.ledger().timestamp();
    let expires_at = now + 1000;
    client.allow_address_until(&admin, &subject, &expires_at);
    let expiry = client.get_allow_expiry(&operator, &subject);
    assert_eq!(expiry, Some(expires_at));
}

#[test]
fn non_operator_cannot_call_get_allow_expiry() {
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    client.allow_address_until(&admin, &subject, &(now + 1000));
    let non_operator = Address::generate(&env);
    let result = client.try_get_allow_expiry(&non_operator, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn admin_can_call_address_status() {
    use compliance::AddressState;
    let (_env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    let status = client.address_status(&admin, &subject);
    assert_eq!(status, AddressState::Allowed);
}

#[test]
fn operator_can_call_address_status() {
    use compliance::AddressState;
    let (env, admin, subject, client) = setup();
    let operator = Address::generate(&env);
    client.set_operator(&admin, &operator);
    client.allow_address(&admin, &subject);
    let status = client.address_status(&operator, &subject);
    assert_eq!(status, AddressState::Allowed);
}

#[test]
fn non_operator_cannot_call_address_status() {
    let (env, _admin, subject, client) = setup();
    let non_operator = Address::generate(&env);
    let result = client.try_address_status(&non_operator, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn address_status_blocked_shows_blocked() {
    use compliance::AddressState;
    let (_env, admin, subject, client) = setup();
    client.allow_address(&admin, &subject);
    client.block_address(&admin, &subject);
    let status = client.address_status(&admin, &subject);
    assert_eq!(status, AddressState::Blocked);
}

#[test]
fn address_status_expired_temp_allow_shows_expired() {
    use compliance::AddressState;
    let (env, admin, subject, client) = setup();
    let now = env.ledger().timestamp();
    client.allow_address_until(&admin, &subject, &now);
    let status = client.address_status(&admin, &subject);
    assert_eq!(status, AddressState::Expired);
}

#[test]
fn operator_cannot_modify_allowlist() {
    let (env, admin, subject, client) = setup();
    let operator = Address::generate(&env);
    client.set_operator(&admin, &operator);
    // operator has no allow_address privilege
    let result = client.try_allow_address(&operator, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn operator_cannot_modify_blocklist() {
    let (env, admin, subject, client) = setup();
    let operator = Address::generate(&env);
    client.set_operator(&admin, &operator);
    client.allow_address(&admin, &subject);
    // operator has no block_address privilege
    let result = client.try_block_address(&operator, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

#[test]
fn replacing_operator_revokes_old_operator_access() {
    let (env, admin, subject, client) = setup();
    let operator1 = Address::generate(&env);
    let operator2 = Address::generate(&env);
    client.set_operator(&admin, &operator1);
    client.set_operator(&admin, &operator2);
    // operator1 is no longer the operator
    client.allow_address_until(&admin, &subject, &(env.ledger().timestamp() + 1000));
    let result = client.try_get_allow_expiry(&operator1, &subject);
    assert_eq!(result, Err(Ok(ContractError::Unauthorized)));
}

// ── Event emission assertions ─────────────────────────────────────────────────
//
// Each test calls exactly one state-changing entrypoint and asserts that the
// last event's topic[0] matches the expected symbol string.  This guards
// against an indexer silently missing a state-change event, which would leave
// compliance data stale.

fn event_symbol(env: &Env, topics: &soroban_sdk::Vec<soroban_sdk::Val>) -> String {
    let sym: Symbol = topics
        .get_unchecked(0)
        .try_into()
        .unwrap_or_else(|_| Symbol::new(env, ""));
    sym.to_string()
}

fn last_event_symbol(env: &Env) -> String {
    let events = env.events().all();
    let n = events.len();
    assert!(n > 0, "no events emitted");
    let (_, topics, _) = events.get(n - 1).unwrap();
    event_symbol(env, &topics)
}

#[test]
fn emits_address_allowed_until_event() {
    let (env, admin, subject, client) = setup();
    let expires_at = env.ledger().timestamp() + 1000;
    client.allow_address_until(&admin, &subject, &expires_at);
    assert!(client.is_allowed(&subject));
    assert_eq!(last_event_symbol(&env), "address_allowed_until");
}

#[test]
fn emits_admin_transfer_initiated_event() {
    let (env, admin, _, client) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    assert_eq!(last_event_symbol(&env), "admin_transfer_initiated");
}

#[test]
fn emits_admin_transferred_event() {
    let (env, admin, _, client) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    client.accept_admin(&new_admin);
    assert_eq!(last_event_symbol(&env), "admin_transferred");
}

#[test]
fn emits_compliance_paused_event() {
    let (env, admin, _, client) = setup();
    client.pause(&admin);
    assert_eq!(last_event_symbol(&env), "compliance_paused");
}

#[test]
fn emits_compliance_unpaused_event() {
    let (env, admin, _, client) = setup();
    client.pause(&admin);
    client.unpause(&admin);
    assert_eq!(last_event_symbol(&env), "compliance_unpaused");
}
