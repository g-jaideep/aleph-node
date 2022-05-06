#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

// TODO : create ERC20
// TODO : contract holds ERC20 funds
// TODO : contract distributes funds to users accounts (according to a formula)

#[ink::contract]
mod the_button {

    use ink_storage::{traits::SpreadAllocate, Mapping};

    /// Defines the storage
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct TheButton {
        /// block number at which the game ends
        deadline: u32,
        /// Stores a mapping between user accounts and the block number at which they pressed the button
        presses: Mapping<AccountId, u32>,
        /// stores the laast account that pressed the button
        last_presser: AccountId,
    }

    /// Error types.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Returned if given account already pressed the button
        AlreadyParticipated,
        /// Returned if button is pressed after the deadline
        AfterDeadline,
    }

    /// Result type.
    pub type Result<T> = core::result::Result<T, Error>;

    impl TheButton {
        /// Constructor
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|contract: &mut Self| {
                let now = Self::env().block_number();
                contract.deadline = now + 604800u32;
            })
        }

        // TODO
        /// Button press logic
        #[ink(message)]
        pub fn press(&mut self) -> Result<()> {
            let by = self.env().caller();

            if self.presses.get(&by).is_some() {
                return Err(Error::AlreadyParticipated);
            }

            let now = self.env().block_number();

            Ok(())
        }
    }
}
