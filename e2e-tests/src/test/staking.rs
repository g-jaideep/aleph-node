use crate::accounts::get_sudo;
use crate::{
    accounts::{accounts_from_seeds, default_accounts},
    config::Config,
    waiting::{wait_for_event, wait_for_finalized_block},
    BlockNumber, Connection, Header, KeyPair,
};
use codec::{Compact, Decode};
use common::create_connection;
use log::info;
use pallet_staking::{StakingLedger, ValidatorPrefs};
use primitives::{Balance, TOKEN_DECIMALS};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use sp_core::crypto::AccountId32;
use sp_core::sr25519::Public;
use sp_core::Pair;
use sp_runtime::Perbill;
use std::iter;
use substrate_api_client::{
    compose_call, compose_extrinsic, extrinsic::staking::RewardDestination, AccountId,
    GenericAddress, XtStatus,
};

const TOKEN: u128 = 10u128.pow(TOKEN_DECIMALS);
const VALIDATOR_STAKE: u128 = 25_000 * TOKEN;
const NOMINATOR_STAKE: u128 = 1_000 * TOKEN;

fn send_xt(connection: &Connection, xt: String, xt_name: &'static str, tx_status: XtStatus) {
    let block_hash = connection
        .send_extrinsic(xt, tx_status)
        .expect("Could not send extrinsc")
        .expect("Could not get tx hash");
    let block_number = connection
        .get_header::<Header>(Some(block_hash))
        .expect("Could not fetch header")
        .expect("Block exists; qed")
        .number;
    info!(
        "Transaction {} was included in block {}.",
        xt_name, block_number
    );
}

fn endow_stash_balances(connection: &Connection, keys: &[KeyPair], endowment: u128) {
    let batch_endow: Vec<_> = keys
        .iter()
        .map(|key| {
            compose_call!(
                connection.metadata,
                "Balances",
                "transfer",
                GenericAddress::Id(AccountId::from(key.public())),
                Compact(endowment)
            )
        })
        .collect();

    let xt = compose_extrinsic!(connection, "Utility", "batch", batch_endow);
    send_xt(
        connection,
        xt.hex_encode(),
        "batch of endow balances",
        XtStatus::InBlock,
    );
}

fn bond(address: &str, initial_stake: u128, stash: &KeyPair, controller: &KeyPair) {
    let connection = create_connection(address).set_signer(stash.clone());
    let controller_account_id = GenericAddress::Id(AccountId::from(controller.public()));

    let xt = connection.staking_bond(
        controller_account_id,
        initial_stake,
        RewardDestination::Staked,
    );
    send_xt(&connection, xt.hex_encode(), "bond", XtStatus::InBlock);
}

fn bonded(connection: &Connection, stash: &KeyPair) -> Option<AccountId> {
    let account_id = AccountId::from(stash.public());
    connection
        .get_storage_map("Staking", "Bonded", account_id, None)
        .unwrap()
}

fn ledger(
    connection: &Connection,
    controller: &KeyPair,
) -> Option<pallet_staking::StakingLedger<AccountId32, Balance>> {
    let account_id = AccountId::from(controller.public());
    connection
        .get_storage_map("Staking", "Ledger", account_id, None)
        .unwrap()
}

fn validate(address: &str, controller: &KeyPair, tx_status: XtStatus) {
    let connection = create_connection(address).set_signer(controller.clone());
    let prefs = ValidatorPrefs {
        blocked: false,
        commission: Perbill::from_percent(10),
    };

    let xt = compose_extrinsic!(connection, "Staking", "validate", prefs);
    send_xt(&connection, xt.hex_encode(), "validate", tx_status);
}

fn nominate(address: &str, nominator_key_pair: &KeyPair, nominee_key_pair: &KeyPair) {
    let nominee_account_id = AccountId::from(nominee_key_pair.public());
    let connection = create_connection(address).set_signer(nominator_key_pair.clone());

    let xt = connection.staking_nominate(vec![GenericAddress::Id(nominee_account_id)]);
    send_xt(&connection, xt.hex_encode(), "nominate", XtStatus::InBlock);
}

fn payout_stakers(address: &str, validator: KeyPair, era_number: BlockNumber) {
    let account = AccountId::from(validator.public());
    let connection = create_connection(address).set_signer(validator);
    let xt = compose_extrinsic!(connection, "Staking", "payout_stakers", account, era_number);

    send_xt(
        &connection,
        xt.hex_encode(),
        "payout_stakers",
        XtStatus::InBlock,
    );
}

fn wait_for_full_era_completion(connection: &Connection) -> anyhow::Result<BlockNumber> {
    let sessions_per_era: u32 = connection
        .get_storage_value("Elections", "SessionsPerEra", None)
        .unwrap()
        .unwrap();
    let current_era: u32 = connection
        .get_storage_value("Staking", "ActiveEra", None)
        .unwrap()
        .unwrap();
    let payout_era = current_era + 2;

    let first_session_in_payout_era = payout_era * sessions_per_era;

    info!(
        "Current era: {}, waiting for the first session in the payout era {}",
        current_era, first_session_in_payout_era
    );

    #[derive(Debug, Decode, Clone)]
    struct NewSessionEvent {
        session_index: u32,
    }
    wait_for_event(
        connection,
        ("Session", "NewSession"),
        |e: NewSessionEvent| {
            info!("[+] new session {}", e.session_index);

            e.session_index == first_session_in_payout_era
        },
    )?;

    Ok(payout_era)
}

fn get_key_pairs() -> (Vec<KeyPair>, Vec<KeyPair>) {
    let validators = default_accounts();
    let validator_stashes: Vec<_> = validators
        .iter()
        .map(|v| String::from(v) + "//stash")
        .collect();
    let validator_accounts_key_pairs = accounts_from_seeds(Some(&validators));
    let stashes_accounts_key_pairs = accounts_from_seeds(Some(&validator_stashes));

    (stashes_accounts_key_pairs, validator_accounts_key_pairs)
}

// 1. endow stash accounts balances, controller accounts are already endowed in chainspec
// 2. bond controller account to stash account, stash = controller and set controller to StakerStatus::Validate
// 3. bond controller account to stash account, stash = controller and set controller to StakerStatus::Nominate
// 4. wait for new era
// 5. send payout stakers tx
pub fn staking_era_payouts(config: &Config) -> anyhow::Result<()> {
    let (stashes_accounts, validator_accounts) = get_key_pairs();

    let node = &config.node;
    let sender = validator_accounts[0].clone();
    let connection = create_connection(node).set_signer(sender);

    endow_stash_balances(&connection, &stashes_accounts, VALIDATOR_STAKE);

    validator_accounts.par_iter().for_each(|account| {
        bond(node, VALIDATOR_STAKE, &account.clone(), account);
    });

    validator_accounts
        .par_iter()
        .for_each(|account| validate(node, account, XtStatus::InBlock));

    stashes_accounts
        .par_iter()
        .for_each(|nominator| bond(node, NOMINATOR_STAKE, &nominator.clone(), nominator));

    stashes_accounts
        .par_iter()
        .zip(validator_accounts.par_iter())
        .for_each(|(nominator, nominee)| nominate(node, nominator, nominee));

    // All the above calls influace the next era, so we need to wait that it passes.
    let current_era = wait_for_full_era_completion(&connection)?;
    info!(
        "Era {} started, claiming rewards for era {}",
        current_era,
        current_era - 1
    );

    validator_accounts
        .into_par_iter()
        .for_each(|account| payout_stakers(node, account, current_era - 1));

    // Sanity check
    let block_number = connection
        .get_header::<Header>(None)
        .unwrap()
        .unwrap()
        .number;
    info!(
        "Current block number is {}, waiting till it finalizes",
        block_number,
    );

    wait_for_finalized_block(&connection, block_number)?;

    Ok(())
}

// 1. endow stash account balances
// 2. bond controller account to the stash account, stash != controller and set controller to StakerStatus::Validate
// 3. call bonded double check bonding
// 4. set controller to StakerStatus::Validate
// 5. call ledger to double check previous action
pub fn staking_new_validator(config: &Config) -> anyhow::Result<()> {
    let (stashes_accounts, validator_accounts) = get_key_pairs();
    let stash_account = stashes_accounts[0].clone();
    let validator_account = validator_accounts[0].clone();
    let validator_account_id = AccountId::from(validator_account.public());
    let stash_account_id = AccountId::from(stash_account.public());
    assert_ne!(stash_account_id, validator_account_id);

    let node = &config.node;
    let sender = validator_accounts[0].clone();
    let connection = create_connection(node).set_signer(sender);

    // to cover tx fees as we need minimal exactly VALIDATOR_STAKE
    endow_stash_balances(
        &connection,
        &[stash_account.clone()],
        VALIDATOR_STAKE + TOKEN,
    );

    bond(node, VALIDATOR_STAKE, &stash_account, &validator_account);
    let bonded_controller_account_ids = bonded(&connection, &stash_account);
    assert!(
        bonded_controller_account_ids.is_some(),
        "Expected that stash account {} is bonded to some controller!",
        &validator_account_id
    );
    let bonded_controller_account_ids = bonded_controller_account_ids.unwrap();
    assert_eq!(
        bonded_controller_account_ids, validator_account_id,
        "Expected that stash account {} is bonded to controller account {}!",
        &stash_account_id, &validator_account_id
    );

    // TODO after this call results in UI seems very weird: stash_account_id seems to be waiting
    // to be elected in next era instead of expected validator_account_id
    validate(node, &validator_account, XtStatus::Finalized);

    let ledger = ledger(&connection, &validator_account);
    assert!(
        ledger.is_some(),
        "Expected controller {} configuration to be non empty",
        validator_account_id
    );
    let ledger = ledger.unwrap();
    assert_eq!(
        ledger,
        StakingLedger {
            stash: stash_account_id.clone(),
            total: VALIDATOR_STAKE,
            active: VALIDATOR_STAKE,
            unlocking: vec![],
            claimed_rewards: vec![]
        }
    );

    // TODO call change_validators to check whether electing stash_account_id could work

    // All the above calls influace the next era, so we need to wait that it passes.
    let current_era = wait_for_full_era_completion(&connection)?;
    info!(
        "Era {} started, claiming rewards for era {}",
        current_era,
        current_era - 1
    );

    // TODO below does nothing, payout_stakers seems to be done but with no visible effect
    payout_stakers(node, stash_account, current_era - 1);

    // // TODO refactor below code and one in validators_change.rs
    // let sudo = get_sudo(config);
    //
    // let connection = create_connection(node).set_signer(sudo);
    //
    // let members_before: Vec<AccountId> = connection
    //     .get_storage_value("Elections", "Members", None)?
    //     .unwrap();
    //
    // info!("[+] members before tx: {:#?}", members_before);
    //
    // let new_members: Vec<AccountId> = accounts
    //     .iter()
    //     .map(|pair| pair.public().into())
    //     .chain(iter::once(
    //         AccountId::from_ss58check("5EHkv1FCd4jeQmVrbYhrETL1EAr8NJxNbukDRT4FaYWbjW8f").unwrap(),
    //     ))
    //     .collect();
    //
    // info!("[+] New members {:#?}", new_members);
    //
    // let call = compose_call!(
    //     connection.metadata,
    //     "Elections",
    //     "change_members",
    //     new_members.clone()
    // );
    //
    // let tx = compose_extrinsic!(connection, "Sudo", "sudo_unchecked_weight", call, 0_u64);
    //
    // // send and watch extrinsic until finalized
    // let tx_hash = connection
    //     .send_extrinsic(tx.hex_encode(), XtStatus::InBlock)
    //     .expect("Could not send extrinsc")
    //     .expect("Could not get tx hash");
    //
    // info!("[+] change_members transaction hash: {}", tx_hash);

    Ok(())
}
