#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

/// This is the YellowButton
/// Rewards are distributed for extending the life of the button for as long as possible:
/// user_score = deadline - now
/// Pressiah gets 50% of tokens
/// the game is played until TheButton dies

// DONE : contract holds ERC20 funds
// DONE : contract distributes funds to all accounts that participated (according to a formula)
// e.g. :
// - 50% go to the Pressiah
// - rest is distributed proportionally to how long has a given user extended TheButtons life for
// IN-PROGRESS : add sybil protection (only whitelisted accounts can participate)
// - TODO add / remove whitelisted accounts
// TODO : add getters
// maybe TODO : add upgradeability (proxy)

#[ink::contract]
mod yellow_button {

    use ink_env::{
        call::{build_call, Call, ExecutionInput, Selector},
        DefaultEnvironment, Error as InkEnvError,
    };
    use ink_prelude::{string::String, vec::Vec};
    use ink_storage::{traits::SpreadAllocate, Mapping};

    /// How many blocks does The Button live for
    const BUTTON_LIFETIME: u32 = 604800; // 7 days assuming 1s block time

    /// Error types
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Returned if given account already pressed The Button
        AlreadyParticipated,
        /// Returned if button is pressed after the deadline
        AfterDeadline,
        /// Account not whitelisted to play
        NotWhitelisted,
        /// Returned if a call to another contract has failed
        ContractCall(String),
    }

    /// Result type
    pub type Result<T> = core::result::Result<T, Error>;

    impl From<InkEnvError> for Error {
        fn from(e: InkEnvError) -> Self {
            match e {
                InkEnvError::Decode(_e) => {
                    Error::ContractCall(String::from("Contract call failed due to Decode error"))
                }
                InkEnvError::CalleeTrapped => Error::ContractCall(String::from(
                    "Contract call failed due to CalleeTrapped error",
                )),
                InkEnvError::CalleeReverted => Error::ContractCall(String::from(
                    "Contract call failed due to CalleeReverted error",
                )),
                InkEnvError::KeyNotFound => Error::ContractCall(String::from(
                    "Contract call failed due to KeyNotFound error",
                )),
                InkEnvError::_BelowSubsistenceThreshold => Error::ContractCall(String::from(
                    "Contract call failed due to _BelowSubsistenceThreshold error",
                )),
                InkEnvError::TransferFailed => Error::ContractCall(String::from(
                    "Contract call failed due to TransferFailed error",
                )),
                InkEnvError::_EndowmentTooLow => Error::ContractCall(String::from(
                    "Contract call failed due to _EndowmentTooLow error",
                )),
                InkEnvError::CodeNotFound => Error::ContractCall(String::from(
                    "Contract call failed due to CodeNotFound error",
                )),
                InkEnvError::NotCallable => Error::ContractCall(String::from(
                    "Contract call failed due to NotCallable error",
                )),
                InkEnvError::Unknown => {
                    Error::ContractCall(String::from("Contract call failed due to Unknown error"))
                }
                InkEnvError::LoggingDisabled => Error::ContractCall(String::from(
                    "Contract call failed due to LoggingDisabled error",
                )),
                InkEnvError::EcdsaRecoveryFailed => Error::ContractCall(String::from(
                    "Contract call failed due to EcdsaRecoveryFailed error",
                )),
                #[cfg(any(feature = "std", test, doc))]
                InkEnvError::OffChain(_e) => {
                    Error::ContractCall(String::from("Contract call failed due to OffChain error"))
                }
            }
        }
    }

    /// Defines the storage
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct YellowButton {
        /// is The Button dead
        is_dead: bool,
        /// block number at which the game ends
        deadline: u32,
        /// Stores a mapping between user accounts and the number of blocks they extended The Buttons life for
        presses: Mapping<AccountId, u32>,
        /// stores keys to `presses` because Mapping is not an Iterator. Heap-allocated! so we might need Map<u32, AccountId>
        press_accounts: Vec<AccountId>,
        /// stores total sum of user scores
        total_scores: u32,
        /// stores the last account that pressed The Button
        last_presser: Option<AccountId>,
        /// block number of the last press
        last_press: u32,
        /// AccountId of the ERC20 ButtonToken instance on-chain
        button_token: AccountId,
        /// accounts whitelisted to play the game
        can_play: Mapping<AccountId, bool>,
    }

    /// Event emitted when The Button is pressed
    #[ink(event)]
    pub struct ButtonPressed {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        when: u32,
    }

    impl YellowButton {
        /// Constructor
        #[ink(constructor)]
        pub fn new(button_token: AccountId) -> Self {
            ink_lang::utils::initialize_contract(|contract: &mut Self| {
                let now = Self::env().block_number();
                contract.is_dead = false;
                contract.deadline = now + BUTTON_LIFETIME;
                contract.button_token = button_token;
            })
        }

        /// End of the game logic
        fn death(&mut self) -> Result<()> {
            self.is_dead = true;

            let this = self.env().account_id();
            let button_token = self.button_token;

            let total_balance = build_call::<DefaultEnvironment>()
                .call_type(Call::new().callee(button_token).gas_limit(5000))
                .transferred_value(self.env().transferred_value())
                .exec_input(
                    ExecutionInput::new(
                        Selector::new([0, 0, 0, 2]), // balance_of
                    )
                    .push_arg(this),
                )
                .returns::<Balance>()
                .fire()?;

            // Pressiah gets 50% of supply
            let pressiah_reward = total_balance / 2;
            if let Some(pressiah) = self.last_presser {
                let _ = build_call::<DefaultEnvironment>()
                    .call_type(Call::new().callee(button_token).gas_limit(5000))
                    .transferred_value(self.env().transferred_value())
                    .exec_input(
                        ExecutionInput::new(
                            Selector::new([0, 0, 0, 4]), // transfer
                        )
                        .push_arg(pressiah)
                        .push_arg(pressiah_reward),
                    )
                    .returns::<()>()
                    .fire()?;
            }

            let total = self.total_scores;
            let remaining_balance = total_balance - pressiah_reward;
            // rewards are distributed to participants proportionally to their score
            let _ = self
                .press_accounts
                .iter()
                .try_for_each(|account_id| -> Result<()> {
                    if let Some(score) = self.presses.get(account_id) {
                        let reward = (score / total) as u128 * remaining_balance;

                        // transfer amount
                        return Ok(build_call::<DefaultEnvironment>()
                            .call_type(Call::new().callee(button_token).gas_limit(5000))
                            .transferred_value(self.env().transferred_value())
                            .exec_input(
                                ExecutionInput::new(
                                    Selector::new([0, 0, 0, 4]), // transfer
                                )
                                .push_arg(account_id)
                                .push_arg(reward),
                            )
                            .returns::<()>()
                            .fire()?);
                    }
                    Ok(())
                });

            Ok(())
        }

        // TODO : ownership
        /// Whitelists given AccountId to participate in the game
        ///
        #[ink(message)]
        pub fn allow(&mut self, player: AccountId) -> Result<()> {
            self.can_play.insert(player, &true);
            Ok(())
        }

        // TODO : ownership
        /// Blacklists given AccountId from participating in the game
        ///
        #[ink(message)]
        pub fn disallow(&mut self, player: AccountId) -> Result<()> {
            self.can_play.insert(player, &false);
            Ok(())
        }

        /// Button press logic
        #[ink(message)]
        pub fn press(&mut self) -> Result<()> {
            if self.is_dead {
                return Err(Error::AfterDeadline);
            }

            let now = self.env().block_number();
            if self.deadline >= now {
                // trigger Buttons death
                return self.death();
            }

            let caller = self.env().caller();
            if self.presses.get(&caller).is_some() {
                return Err(Error::AlreadyParticipated);
            }

            if !self.can_play.get(&caller).unwrap_or(false) {
                return Err(Error::NotWhitelisted);
            }

            // record press
            // score is the number of blocks the button life was extended for
            // this incentivizes pressing as late as possible in the game (but not too late)
            let score = now - self.last_press;
            self.presses.insert(&caller, &score);
            self.press_accounts.push(caller);
            // another
            self.last_presser = Some(caller);
            self.last_press = now;
            self.total_scores += score;
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
