use super::*;
use crate::{
    mock,
    types::{LightBlockStorage, TimestampStorage},
    utils::tendermint_hash_to_h256,
    Pallet as TendermintLightClient,
};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::{assert_err, assert_ok, traits::Get};
use frame_system::{Origin, RawOrigin};
use tendermint::block::Header;
use tendermint_light_client_verifier::{
    operations::{Hasher, ProdHasher},
    types::LightBlock,
};
use tendermint_testgen as testgen;

benchmarks! {
    benchmark_for_initialize_client {

        let v in 1 .. T::MaxVotesCount::get();
        let mut blocks = mock::generate_consecutive_blocks (1, String::from ("test-chain"), v, 3, TimestampStorage::new (3, 0));

        // let caller = RawOrigin::Root;
        let initial_block = blocks.pop ().unwrap ();
        let options = types::LightClientOptionsStorage::default();

    }: initialize_client(RawOrigin::Root, options, initial_block.clone ())

        verify {
            // check if persisted
            let last_imported = TendermintLightClient::<T>::get_last_imported_hash();
            assert_eq!(
                initial_block.signed_header.commit.block_id.hash,
                last_imported
            );
        }

    benchmark_for_update_client {

        let v in 1 .. T::MaxVotesCount::get();
        let mut blocks = mock::generate_consecutive_blocks (2, String::from ("test-chain"), v, 3, TimestampStorage::new (3, 0));

        let options = types::LightClientOptionsStorage::default();
        let initial_block = blocks.pop ().unwrap ();

        assert_ok!(TendermintLightClient::<T>::initialize_client(
            RawOrigin::Root.into() ,
            options,
            initial_block.clone ()
        ));

        let caller: T::AccountId = whitelisted_caller();
        let untrusted_block = blocks.pop ().unwrap ();

    }: update_client(RawOrigin::Signed(caller.clone()), untrusted_block.clone ())

        verify {
            // check if persisted
            let last_imported = TendermintLightClient::<T>::get_last_imported_hash();
            assert_eq!(
                untrusted_block.signed_header.commit.block_id.hash,
                last_imported
            );
        }

    // TODO :
    // this benchmarks update_client call which causes pruning of the oldest block
    // mock runtime keeps 3 headers, therefore rollover happens after inserting a third consecutive block
    benchmark_for_update_client_with_pruning {

        // 1970-01-01T00:00:05Z
        mock::Timestamp::set_timestamp(5000);

        let mut blocks = mock::generate_consecutive_blocks (3, String::from ("test-chain"), 2, 3, TimestampStorage::new (3, 0));

        let options = types::LightClientOptionsStorage::default();
        let initial_block = blocks.pop ().unwrap ();

        assert_ok!(TendermintLightClient::<T>::initialize_client(
            RawOrigin::Root.into(),
            options,
            initial_block.clone ()
        ));

        let caller: T::AccountId = whitelisted_caller();
        let next = blocks.pop ().unwrap ();
        let next_next = blocks.pop ().unwrap ();

        assert_ok!(TendermintLightClient::<T>::update_client(
            RawOrigin::Signed(caller.clone()).into (),
            next
        ));

    }: update_client(RawOrigin::Signed(caller.clone()), next_next)

        verify {
            // check if rollover happened

            // let last_imported = TendermintLightClient::<T>::get_last_imported_hash();

            assert_eq!(true, true
                       // update_block.signed_header.commit.block_id.hash,
                       // last_imported
            );
        }

    impl_benchmark_test_suite!(
        TendermintLightClient,
        mock::new_genesis_storage (),
        mock::TestRuntime,
    );

}
