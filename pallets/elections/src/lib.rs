//! This pallet manages changes in the committee responsible for producing blocks and establishing consensus.
//! Currently, it's PoA where the validators are set by the root account. In the future, a new
//! version for DPoS elections will replace the current one.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use frame_support::traits::StorageVersion;
pub use pallet::*;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_election_provider_support::{
        ElectionDataProvider, ElectionProvider, Support, Supports,
    };
    use frame_support::{pallet_prelude::*, traits::Get};
    use frame_system::{
        ensure_root,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use sp_std::{collections::btree_map::BTreeMap, prelude::Vec};

    #[pallet::storage]
    #[pallet::getter(fn members)]
    pub type Members<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type DataProvider: ElectionDataProvider<Self::AccountId, Self::BlockNumber>;
        #[pallet::constant]
        type SessionPeriod: Get<u32>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ChangeMembers(Vec<T::AccountId>),
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn change_members(origin: OriginFor<T>, members: Vec<T::AccountId>) -> DispatchResult {
            ensure_root(origin)?;
            Members::<T>::put(members.clone());
            Self::deposit_event(Event::ChangeMembers(members));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub members: Vec<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                members: Vec::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <Members<T>>::put(&self.members);
        }
    }

    impl<T: Config> Pallet<T> {}

    #[derive(Debug)]
    pub enum Error {
        DataProvider(&'static str),
    }

    impl<T: Config> ElectionProvider<T::AccountId, BlockNumberFor<T>> for Pallet<T> {
        type Error = Error;
        type DataProvider = T::DataProvider;

        // The elections are PoA so only the nodes listed in the Members will be elected as validators.
        // We calculate the supports for them for the sake of eras payouts.
        fn elect() -> Result<Supports<T::AccountId>, Self::Error> {
            Self::do_elect()
        }
    }

    impl<T: Config> Pallet<T> {
        fn do_elect() -> Result<Supports<T::AccountId>, Error> {
            let voters = T::DataProvider::voters(None).map_err(Error::DataProvider)?;
            let members = Pallet::<T>::members();
            let mut supports: BTreeMap<T::AccountId, Support<T::AccountId>> = members
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        Support {
                            total: 0,
                            voters: Vec::new(),
                        },
                    )
                })
                .collect();

            for (voter, vote, targets) in voters {
                // The parameter Staking::MAX_NOMINATIONS is set to 1 which guarantees that len(targets) == 1
                let member = &targets[0];
                if let Some(support) = supports.get_mut(member) {
                    support.total += vote as u128;
                    support.voters.push((voter, vote as u128));
                }
            }

            Ok(supports.into_iter().collect())
        }

        fn do_elect_vec() -> Result<Supports<T::AccountId>, Error> {
            let voters = T::DataProvider::voters(None).map_err(Error::DataProvider)?;
            let members = Pallet::<T>::members();
            let mut supports: Vec<(T::AccountId, Support<T::AccountId>)> = members
                .into_iter()
                .map(|id| {
                    (
                        id,
                        Support {
                            total: 0,
                            voters: Vec::new(),
                        },
                    )
                })
                .collect();

            for (voter, vote, targets) in voters {
                // The parameter Staking::MAX_NOMINATIONS is set to 1 which guarantees that len(targets) == 1
                let member = &targets[0];
                if let Some(support) = supports.iter_mut().find(|(acc_id, _supp)| acc_id == member)
                {
                    support.1.total += vote as u128;
                    support.1.voters.push((voter, vote as u128));
                }
            }
            Ok(supports)
        }

        fn do_elect_vec_bs() -> Result<Supports<T::AccountId>, Error> {
            let voters = T::DataProvider::voters(None).map_err(Error::DataProvider)?;
            let mut members = Pallet::<T>::members();
            members.sort_unstable();

            let mut supports: Vec<(T::AccountId, Support<T::AccountId>)> = members
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        Support {
                            total: 0,
                            voters: Vec::new(),
                        },
                    )
                })
                .collect();

            for (voter, vote, targets) in voters {
                // The parameter Staking::MAX_NOMINATIONS is set to 1 which guarantees that len(targets) == 1
                let member = &targets[0];
                if let Ok(pos) = members.binary_search(member) {
                    let mut support = &mut supports[pos];
                    support.1.total += vote as u128;
                    support.1.voters.push((voter, vote as u128));
                }
            }
            Ok(supports)
        }

        fn do_elect_fast() -> Result<Supports<T::AccountId>, Error> {
            let voters = T::DataProvider::voters(None).map_err(Error::DataProvider)?;
            let mut members = Pallet::<T>::members();
            members.sort_unstable();
            let mut voters: Vec<_> = voters
                .into_iter()
                .map(|(voter, vote, mut validators)| (validators.remove(0), voter, vote))
                .collect();
            voters.sort_unstable_by(|(validator_a, _, _), (validator_b, _, _)| {
                validator_a.cmp(validator_b)
            });
            let mut supports: Vec<(T::AccountId, Support<T::AccountId>)> = Vec::new();
            let mut ind_start = 0;
            while ind_start < voters.len() {
                let mut ind_end = ind_start + 1;
                while ind_end < voters.len() && voters[ind_start].0 == voters[ind_end].0 {
                    ind_end += 1;
                }
                let validator = &voters[ind_start].0;
                if members.binary_search(validator).is_ok() {
                    let mut sum_votes = 0;
                    let supporters = voters[ind_start..ind_end]
                        .iter()
                        .map(|(_, voter, vote)| {
                            sum_votes += *vote as u128;
                            (voter.clone(), *vote as u128)
                        })
                        .collect();
                    supports.push((
                        validator.clone(),
                        Support {
                            total: sum_votes,
                            voters: supporters,
                        },
                    ));
                }
                ind_start = ind_end;
            }
            Ok(supports)
        }
    }
}
