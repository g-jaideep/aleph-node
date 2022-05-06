#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

// TODO : getters
// TODO : create ERC20
// TODO : contract holds ERC20 funds
// TODO : contract distributes funds to all accounts that participated (according to a formula)
// e.g. :
// - 50% go to the Pressiah
// - rest is distributed proportionally to how long has a given user extended TheButtons life for
// TODO : add upgardeability (proxy)

#[ink::contract]
mod the_button {

    use ink_storage::{traits::SpreadAllocate, Mapping};

    /// Result type
    pub type Result<T> = core::result::Result<T, Error>;
    /// How many blocks does the button live for
    const BUTTON_LIFETIME: u32 = 604800; // 7 days assuming 1s block time

    /// Defines the storage
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct TheButton {
        /// is The Button dead
        is_dead: bool,
        /// block number at which the game ends
        deadline: u32,
        /// Stores a mapping between user accounts and the block number of blocks they extended th ebutton life for
        presses: Mapping<AccountId, u32>,
        /// stores the laast account that pressed the button
        last_presser: AccountId,
    }

    /// Error types
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Returned if given account already pressed the button
        AlreadyParticipated,
        /// Returned if button is pressed after the deadline
        AfterDeadline,
    }

    /// Event emitted when The Button is pressed
    #[ink(event)]
    pub struct ButtonPressed {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        when: u32,
    }

    impl TheButton {
        /// Constructor
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|contract: &mut Self| {
                let now = Self::env().block_number();
                contract.deadline = now + BUTTON_LIFETIME;
            })
        }

        // TODO
        /// End of the game logic
        fn death(&mut self) -> Result<()> {
            todo!()
        }

        /// Button press logic
        #[ink(message)]
        pub fn press(&mut self) -> Result<()> {
            if self.is_dead {
                return Err(Error::AfterDeadline);
            }

            let now = self.env().block_number();
            if self.deadline >= now {
                return self.death();
            }

            let caller = self.env().caller();
            if self.presses.get(&caller).is_some() {
                return Err(Error::AlreadyParticipated);
            }

            // record press
            self.presses.insert(&caller, &(self.deadline - now));
            self.last_presser = caller;

            // reset button lifetime
            self.deadline = now + BUTTON_LIFETIME;

            // emit event
            self.env().emit_event(ButtonPressed {
                from: caller,
                when: now,
            });

            Ok(())
        }
    }
}
