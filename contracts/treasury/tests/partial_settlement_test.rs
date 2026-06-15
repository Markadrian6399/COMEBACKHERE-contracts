use soroban_sdk::{testutils::Address as _, Address, Env};
use treasury::{SettlementStatus, TreasuryContract, TreasuryContractClient};

fn setup(env: &Env, total: i128) -> (TreasuryContractClient, Address, Address, u64) {
    let admin = Address::generate(env);
    let merchant = Address::generate(env);
    let contract_id = env.register_contract(None, TreasuryContract);
    let client = TreasuryContractClient::new(env, &contract_id);
    // threshold=1, admin weight=1 → admin approval alone is sufficient
    client.initialize(&admin, &1);

    let token_id = env.register_stellar_asset_contract(admin.clone());
    soroban_sdk::token::StellarAssetClient::new(env, &token_id).mint(&contract_id, &total);

    let sid = client.propose_settlement(&admin, &merchant, &total);
    (client, admin, token_id, sid)
}

#[test]
fn partially_execute_sets_partial_status() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_id, sid) = setup(&env, 10_000_000);

    client.partially_execute_settlement(&admin, &sid, &3_000_000, &token_id);
    let s = client.get_settlement(&sid);
    assert_eq!(s.status, SettlementStatus::PartiallyExecuted);
}

#[test]
fn partially_executed_settlement_absent_from_pending_list() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_id, sid) = setup(&env, 10_000_000);

    client.partially_execute_settlement(&admin, &sid, &3_000_000, &token_id);
    let pending = client.get_pending_settlements();
    assert_eq!(pending.len(), 0);
}

#[test]
#[should_panic(expected = "ThresholdNotMet")]
fn partially_execute_without_sufficient_approvals_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let contract_id = env.register_contract(None, TreasuryContract);
    let client = TreasuryContractClient::new(&env, &contract_id);
    client.initialize(&admin, &10); // threshold=10, admin weight=1
    let token_id = env.register_stellar_asset_contract(admin.clone());
    soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&contract_id, &1_000_000);
    let sid = client.propose_settlement(&admin, &merchant, &1_000_000);
    client.partially_execute_settlement(&admin, &sid, &500_000, &token_id);
}

#[test]
#[should_panic(expected = "InvalidAmount")]
fn partially_execute_exceeding_amount_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_id, sid) = setup(&env, 1_000_000);

    client.partially_execute_settlement(&admin, &sid, &2_000_000, &token_id);
}

#[test]
#[should_panic(expected = "InvalidAmount")]
fn partially_execute_zero_amount_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_id, sid) = setup(&env, 1_000_000);

    client.partially_execute_settlement(&admin, &sid, &0, &token_id);
}

#[test]
#[should_panic(expected = "AlreadyExecuted")]
fn partially_execute_already_executed_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, token_id, sid) = setup(&env, 1_000_000);

    client.partially_execute_settlement(&admin, &sid, &500_000, &token_id);
    // second call should panic — settlement is now PartiallyExecuted, not Pending
    client.partially_execute_settlement(&admin, &sid, &200_000, &token_id);
}
