use crate::{Config, ErasMembers, MembersPerSession, NonReservedMembers, ReservedMembers};
use frame_support::{
    generate_storage_alias, log,
    traits::{Get, PalletInfoAccess, StorageVersion},
    weights::Weight,
};
use sp_std::vec::Vec;

generate_storage_alias!(
    Elections, Members<T: Config> => Value<Vec<T::AccountId>>
);

/// The assumptions made by this migration:
///
/// There is one storage in the pallet elections `Members` containing current set of validators.
/// After migration the state should be as follows:
/// - `Members` are no longer present.
/// - `MembersPerSession` is u32 storage set to size of the `Members` set.
/// - `ReservedMembers` contains the content of the `Members`
/// - `NonReservedMembers` are empty
/// - `ErasMembers` contain tuple of (content of `Members`, empty vector).
pub fn migrate<T: Config, P: PalletInfoAccess>() -> Weight {
    log::info!(target: "pallet_elections", "Running migration from STORAGE_VERSION 0 to 1");

    let members = Members::<T>::get().expect("Members should be present");
    Members::<T>::kill();

    let members_per_session = members.len() as u32;

    MembersPerSession::<T>::put(members_per_session);
    ReservedMembers::<T>::put(members.clone());
    NonReservedMembers::<T>::put(Vec::<T::AccountId>::new());
    ErasMembers::<T>::put((members, Vec::<T::AccountId>::new()));

    StorageVersion::new(1).put::<P>();
    T::DbWeight::get().reads(1) + T::DbWeight::get().writes(5)
}
