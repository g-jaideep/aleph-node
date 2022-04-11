#![cfg(test)]
extern crate test;

use frame_election_provider_support::SortedListProvider;
use pallet_staking::Nominations;
use test::Bencher;

use crate::mock::*;

const NOMINATOR_COUNT: u64 = 10_000;

fn test_nomination() -> Nominations<AccountId> {
    Nominations {
        targets: Vec::new(),
        submitted_in: 0,
        suppressed: false,
    }
}

fn init_nominators() {
    let n = test_nomination();
    (0..NOMINATOR_COUNT).for_each(|i| {
        pallet_staking::Nominators::<Test>::insert(i, n.clone());
        <Test as pallet_staking::Config>::SortedListProvider::on_insert(i, 0)
            .expect("should succeed");
    });

    assert_eq!(Staking::nominators(0).unwrap(), n);
}

#[bench]
fn bench_nominators_iter(b: &mut Bencher) {
    new_test_ext().execute_with(|| {
        init_nominators();
        b.iter(|| {
            let nominators = pallet_staking::Nominators::<Test>::iter_keys();
            assert_eq!(nominators.count(), NOMINATOR_COUNT as usize);
        })
    });
}

#[bench]
fn bench_bags_iter(b: &mut Bencher) {
    new_test_ext().execute_with(|| {
        init_nominators();
        b.iter(|| {
            let nominators = <Test as pallet_staking::Config>::SortedListProvider::iter();
            assert_eq!(nominators.count(), NOMINATOR_COUNT as usize);
        })
    });
}
