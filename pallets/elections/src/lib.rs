//! This pallet manages changes in the committee responsible for producing blocks and establishing consensus.
//! Currently, it's PoA where the validators are set by the root account. In the future, a new
//! version for DPoS elections will replace the current one.
//!
//! ### Terminology
//! For definition of session, era, staking see pallet_session and pallet_staking.
//! - Committee: Set of nodes that produce and finalize blocks in the era.
//! - Validator: Node that can become a member of committee (or already is) via rotation.
//! - (TODO: remove this to remove confusion) Member: Usually same as validator, sometimes means member of the committee
//! - ReservedValidators: Validators that are chosen to be in committee every single session.

#![cfg_attr(not(feature = "std"), no_std)]

mod impls;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod traits;

use codec::{Decode, Encode};
use frame_support::traits::StorageVersion;
use scale_info::TypeInfo;
use sp_std::{collections::btree_map::BTreeMap, prelude::Vec};

pub use pallet::*;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

pub type BlockCount = u32;
pub type TotalReward = u32;

#[derive(Decode, Encode, TypeInfo)]
pub struct ValidatorTotalRewards<T>(pub BTreeMap<T, TotalReward>);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::traits::{EraInfoProvider, SessionInfoProvider, ValidatorRewardsHandler};
    use frame_election_provider_support::{
        ElectionDataProvider, ElectionProvider, Support, Supports,
    };
    use frame_support::{pallet_prelude::*, traits::Get};
    use frame_system::{ensure_root, pallet_prelude::OriginFor};
    use pallet_session::SessionManager;
    use primitives::DEFAULT_COMMITTEE_SIZE;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Something that provides information about ongoing eras.
        type EraInfoProvider: EraInfoProvider;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Something that provides data for elections.
        type DataProvider: ElectionDataProvider<
            AccountId = Self::AccountId,
            BlockNumber = Self::BlockNumber,
        >;
        /// Nr of blocks in the session.
        #[pallet::constant]
        type SessionPeriod: Get<u32>;
        /// Handler for managing new session.
        type SessionManager: SessionManager<<Self as frame_system::Config>::AccountId>;
        /// Something that provides information about sessions.
        type SessionInfoProvider: SessionInfoProvider<Self>;
        /// Something that handles addition of rewards for validators.
        type ValidatorRewardsHandler: ValidatorRewardsHandler<Self>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ChangeMembers(Vec<T::AccountId>),
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// List of possible validators, used during elections.
    /// Can be modified via `change_members` call that requires sudo.
    #[pallet::storage]
    #[pallet::getter(fn members)]
    pub type Members<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// Desirable size of a committee. When new session is planned, first reserved validators are
    /// added to the committee. Then remaining slots are filled from total validators list excluding
    /// reserved validators
    #[pallet::storage]
    pub type CommitteeSize<T> = StorageValue<_, u32, ValueQuery>;

    /// List of reserved validators in force from a new era.
    /// Can be changed via `change_next_era_reserved_validators` call that requires sudo.
    #[pallet::storage]
    pub type NextEraReservedValidators<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// Current's era list of reserved validators. This is populated from `NextEraReservedValidators`
    /// at the time of planning the first session of the era.
    #[pallet::storage]
    pub type CurrentEraReservedValidators<T: Config> =
        StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// Count per validator, how many blocks did the validator produced
    #[pallet::storage]
    pub type SessionValidatorBlockCount<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, BlockCount, ValueQuery>;

    /// Total possible reward per validator for the current era. Scaled to fit in the u32.
    #[pallet::storage]
    pub type ValidatorEraTotalReward<T: Config> =
        StorageValue<_, ValidatorTotalRewards<T::AccountId>, OptionQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn change_members(origin: OriginFor<T>, members: Vec<T::AccountId>) -> DispatchResult {
            ensure_root(origin)?;
            Members::<T>::put(members.clone());
            Self::deposit_event(Event::ChangeMembers(members));

            Ok(())
        }

        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn set_committee_size(origin: OriginFor<T>, committee_size: u32) -> DispatchResult {
            ensure_root(origin)?;
            CommitteeSize::<T>::put(committee_size);

            Ok(())
        }

        #[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
        pub fn change_next_era_reserved_validators(
            origin: OriginFor<T>,
            next_era_reserved_validators: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            NextEraReservedValidators::<T>::put(next_era_reserved_validators);

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub members: Vec<T::AccountId>,
        pub next_era_reserved_validators: Vec<T::AccountId>,
        pub committee_size: u32,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                members: Vec::new(),
                next_era_reserved_validators: Vec::new(),
                committee_size: DEFAULT_COMMITTEE_SIZE,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            <Members<T>>::put(&self.members);
            <CommitteeSize<T>>::put(&self.committee_size);
            <NextEraReservedValidators<T>>::put(&self.next_era_reserved_validators);
        }
    }

    impl<T: Config> Pallet<T> {}

    #[derive(Debug)]
    pub enum Error {
        DataProvider(&'static str),
    }

    impl<T: Config> ElectionProvider for Pallet<T> {
        type AccountId = T::AccountId;
        type BlockNumber = T::BlockNumber;
        type Error = Error;
        type DataProvider = T::DataProvider;

        // The elections are PoA so only the nodes listed in the Members will be elected as validators.
        // We calculate the supports for them for the sake of eras payouts.
        fn elect() -> Result<Supports<T::AccountId>, Self::Error> {
            let voters = Self::DataProvider::electing_voters(None).map_err(Error::DataProvider)?;
            let members = Pallet::<T>::members();
            let mut supports: BTreeMap<_, _> = members
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
    }
}
