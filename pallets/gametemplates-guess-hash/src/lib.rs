//! Game template 1 Guess Hash: Please guess the block hash, the last number is odd or even. Winner gets chips.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch, dispatch::Vec, ensure,
    weights::Weight, Parameter,
};
use frame_system::ensure_signed;
use sp_runtime::{
    traits::{AtLeast32Bit, Bounded},
    DispatchError,
};

extern crate alloc;
use alloc::{format, str, string::*};

// use chips trait
use pallet_chips::{ChipsTrait, ChipsTransfer};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod default_weight;

/// Game detail info
#[derive(Encode, Decode, Default)]
pub struct GameInfo<Owner, BlockNumber, DrawBlockNumber, Amount> {
    /// create game user
    pub owner: Owner,
    /// Create game current block number
    pub block_number: BlockNumber,
    /// Bet block number(draw)
    pub bet_block_number: DrawBlockNumber,
    //// Prize pool amount (the total amount of bets cannot be greater than the prize pool amount)
    pub amount: Amount,
}
/// Bet detail info
#[derive(Encode, Decode, Default, Debug)]
pub struct BetInfo<Account, GameIndex, Amount, GameMode> {
    /// bet user
    pub user: Account,
    /// game index
    pub game_id: GameIndex,
    /// bet amount
    pub amount: Amount,
    /// game mode is odd or even(1 or 2)
    pub game_mode: GameMode,
}
pub trait WeightInfo {
    fn create_game() -> Weight;
    fn bet() -> Weight;
    fn on_finalize(count: u32) -> Weight;
}
pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    type GameIndex: Parameter + AtLeast32Bit + Bounded + Default + Copy;
    type WeightInfo: WeightInfo;
    type Chips: ChipsTrait + ChipsTransfer<Self::AccountId>;
}

/// chips unit type
type ChipBalance<T> = <<T as Config>::Chips as pallet_chips::ChipsTrait>::ChipBalance;

/// Define the game mode
pub type GameMode = u8;
/// Guess the odd number
pub const GAME_MODE_IS_SINGLE: GameMode = 1;
/// Guess the even number
pub const GAME_MODE_IS_DOUBLE: GameMode = 2;

decl_storage! {
    trait Store for Module<T: Config> as GameGuessHashModule {
        /// List of all games, including not yet drawn and already drawn
        pub Games get(fn game_list): map hasher(blake2_128_concat)  T::GameIndex => GameInfo<T::AccountId, T::BlockNumber, T::BlockNumber, ChipBalance<T>>;
        /// List of betting records
        pub BetList get(fn bet_list): map hasher(blake2_128_concat)  T::GameIndex => Vec<BetInfo<T::AccountId, T::GameIndex, ChipBalance<T>, GameMode>>;
        /// The current total number of games
        pub GameCount get(fn game_count): T::GameIndex;
        /// Can use block num to check which games are about to be drawn.
        pub DrawMap get(fn draw_map): map hasher(blake2_128_concat) T::BlockNumber => Vec<T::GameIndex>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Config>::AccountId,
        ChipBalance = ChipBalance<T>,
        GameIndex = <T as Config>::GameIndex,
        BlockNumber = <T as frame_system::Config>::BlockNumber,
        BlockHash = <T as frame_system::Config>::Hash,
    {
        /// Opening (banker, GameIndex, prize pool amount, betting block)
        CreateGame(AccountId, GameIndex, ChipBalance, BlockNumber),
        ///Place a bet (player, game ID, bet amount, 1: odd or 2: even, bet id)
        Bet(AccountId, GameIndex, ChipBalance, GameMode, u32),
        /// The player settles the winning amount (player, game ID, winning amount, betting ID, game result (1: odd or 2: even), drawn Block Hash)
        BettorResult(AccountId, GameIndex, ChipBalance, u32, GameMode, BlockHash),
        /// Game over (the dealer, the game ID, the total amount received by the dealer, the result of the game (1: odd or 2: even), drawn Block Hash)
        GameOver(AccountId, GameIndex, ChipBalance, GameMode, BlockHash),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        NoneValue,
        StorageOverflow,
        GameCountOverflow,
        GameIsNotExist,
        GameModeIsNotExist,
        BalanceNotEnough,
        TransferError,
        BetAmountLimitError,	// The bet amount reaches the upper limit
        GameOver,
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error::<T>;
        fn deposit_event() = default;

        /// create guess hash game
        #[weight = T::WeightInfo::create_game()]
        pub fn create_game(origin, bet_next_few_block: u32, amount: ChipBalance<T>) -> dispatch::DispatchResult {
            let sender = ensure_signed(origin)?;
            Self::_create_game(&sender, bet_next_few_block, amount)?;
            Ok(())
        }

        /// bet guess hash game
        #[weight = T::WeightInfo::bet()]
        pub fn bet(origin, game_id: T::GameIndex, value: ChipBalance<T>, game_mode: GameMode) -> dispatch::DispatchResult {
            let sender = ensure_signed(origin)?;
            Self::_bet(&sender, game_id, value, game_mode)?;
            Ok(())
        }

        fn on_initialize(_now: T::BlockNumber) -> Weight {
            T::WeightInfo::on_finalize(2u32)
        }

        /// When the target block is generated, the game chips are settled according to the game rules on the chain
        fn on_finalize(now: T::BlockNumber) {
            let game_id_list = Self::draw_map(now);
            // ready draw
            if !game_id_list.is_empty() {
                for game_id in game_id_list {

                    // 前一筆交易的block hash
                    let block_hash = <frame_system::Module<T>>::block_hash(now-1u32.into());
                    let game_info = Self::game_list(&game_id);

                    // get winning mode (odd or even)
                    let result_game_mode = Self::get_game_result(block_hash).ok();

                    // Get betting record
                    let bet_list = Self::bet_list(&game_id);

                    // -----------------------Reward distribution-----------------------
                    // Total prize pool
                    let mut owner_pool = game_info.amount;
                    // The total amount the owner will receive
                    let mut owner_get_total_amount = game_info.amount;

                    // owner
                    let owner = game_info.owner;
                    for (k, v) in bet_list.iter().enumerate() {
                        // winner
                        if v.game_mode == result_game_mode.unwrap() {
                            // Return the bettor's principal
                            T::Chips::unreserve(&v.user, v.amount).map_err(|err| debug::error!("err: {:?}", err)).ok();
                            // Owner issues rewards to punters
                            T::Chips::repatriate_reserved(&owner, &v.user, v.amount).map_err(|err| debug::error!("err: {:?}", err)).ok();

                            // Notify the punter to get the amount
                            Self::deposit_event(RawEvent::BettorResult(v.user.clone(), game_id, v.amount * 2u32.into(), k as u32, result_game_mode.unwrap(), block_hash));

                            // Calculate the remaining amount of the prize pool
                            owner_pool-=v.amount;

                            // Owner lost, total get amount decreased
                            owner_get_total_amount-=v.amount;
                        }
                        // loser
                        else{
                            // The bettor issues a reward to the owner
                            T::Chips::repatriate_reserved(&v.user, &owner, v.amount).map_err(|err| debug::error!("err: {:?}", err)).ok();
                            // The owner wins, the total get amount decreases
                            owner_get_total_amount+=v.amount;
                        }

                    }
                    // The remaining amount of the prize pool is returned to the owner
                    T::Chips::unreserve(&owner, owner_pool).map_err(|err| debug::error!("err: {:?}", err)).ok();

                    // Send notification
                    Self::deposit_event(RawEvent::GameOver(owner, game_id, owner_get_total_amount, result_game_mode.unwrap(), block_hash));
                }
            }
        }
    }
}

impl<T: Config> Module<T> {
    /// check will it exceed the compensable amount of the prize pool after placing a bet?
    fn check_bet_over_pool(game_id: T::GameIndex, bet_amount: ChipBalance<T>) -> bool {
        let game_info = Self::game_list(game_id);
        let bet_list = Self::bet_list(game_id);

        // Maximum Compensable Amount of Prize Pool
        let pool_total = game_info.amount;

        // Total bet amount (including quasi bet amount)
        let mut bet_total: ChipBalance<T> = 0u32.into();
        for v in bet_list {
            bet_total += v.amount;
        }
        bet_total += bet_amount;

        // Return true if it will exceed the prize pool
        pool_total < bet_total
    }

    /// Get new game_id
    fn next_game_id() -> sp_std::result::Result<T::GameIndex, DispatchError> {
        let game_id = Self::game_count() + 1u32.into();
        if game_id == T::GameIndex::max_value() {
            return Err(Error::<T>::GameCountOverflow.into());
        }
        Ok(game_id)
    }

    /// create guess hash game
    pub fn _create_game(
        sender: &T::AccountId,
        bet_next_few_block: u32,
        _amount: ChipBalance<T>,
    ) -> sp_std::result::Result<T::GameIndex, DispatchError> {
        // Current transaction block number
        let _block_number = <frame_system::Module<T>>::block_number();
        // Get the Index of the new game
        let game_id = Self::next_game_id()?;

        let _bet_block_number = _block_number + bet_next_few_block.into();
        let game_info = GameInfo {
            owner: sender.clone(),
            block_number: _block_number,
            bet_block_number: _bet_block_number,
            amount: _amount,
        };
        <Games<T>>::insert(&game_id, game_info);
        <GameCount<T>>::put(game_id);

        // The block where the reward is distributed (the next block mined by the betting block is drawn)
        let draw_block_number = _bet_block_number + 1u32.into();
        let mut game_id_list = <DrawMap<T>>::get(&draw_block_number);
        game_id_list.insert(game_id_list.len(), game_id);
        <DrawMap<T>>::insert(&draw_block_number, game_id_list);

        // Pledge now
        T::Chips::reserve(&sender, _amount).map_err(|_| Error::<T>::TransferError)?;

        // Notification of create game
        Self::deposit_event(RawEvent::CreateGame(
            sender.clone(),
            game_id,
            _amount,
            _bet_block_number,
        ));
        Ok(game_id)
    }

    /// bet guess hash game
    pub fn _bet(
        sender: &T::AccountId,
        _game_id: T::GameIndex,
        value: ChipBalance<T>,
        _game_mode: GameMode,
    ) -> dispatch::DispatchResult {
        // Check that GameIndex exists
        ensure!(
            Games::<T>::contains_key(_game_id),
            Error::<T>::GameIsNotExist
        );

        // Check whether the bet game is over
        let game_info = Self::game_list(&_game_id);
        let now_block_number = <frame_system::Module<T>>::block_number();
        ensure!(
            now_block_number < game_info.bet_block_number,
            Error::<T>::GameOver
        );

        // Check the bet amount
        let is_over_pool = Self::check_bet_over_pool(_game_id, value);
        ensure!(!is_over_pool, Error::<T>::BetAmountLimitError);

        // Check game mode
        if _game_mode != GAME_MODE_IS_DOUBLE && _game_mode != GAME_MODE_IS_SINGLE {
            return Err(Error::<T>::GameModeIsNotExist.into());
        }

        // define new betting record
        let new_bet_info = BetInfo {
            user: sender.clone(),
            game_id: _game_id,
            amount: value,
            game_mode: _game_mode,
        };

        // Record new betting records
        let mut bet_list = BetList::<T>::get(_game_id); // Get all betting records
        let bet_index = bet_list.len(); // New bet id
        bet_list.insert(bet_index, new_bet_info); // insert records
        <BetList<T>>::insert(&_game_id, bet_list);

        // Pledge now
        T::Chips::reserve(&sender, value).map_err(|err| err)?;

        // Notification of bet record
        Self::deposit_event(RawEvent::Bet(
            sender.clone(),
            _game_id,
            value,
            _game_mode,
            bet_index as u32,
        ));
        Ok(())
    }

    /// Get the result
    fn get_game_result(block_hash: T::Hash) -> sp_std::result::Result<GameMode, DispatchError> {
        let block_hash_char: String = format!("{:?}", block_hash);
        let char_vec: Vec<char> = block_hash_char.chars().collect();

        let mut is_have_ans = false;
        let mut ans: u8 = 0;
        let mut n = char_vec.len() - 1;
        while !is_have_ans {
            // string to u8
            let num = char_vec[n].to_string().parse::<u8>().ok();
            if num != None {
                ans = num.unwrap();
                is_have_ans = true;
            } else {
            }
            n -= 1;
        }
        // even
        if (ans % 2) == 0 {
            Ok(GAME_MODE_IS_DOUBLE)
        }
        // odd
        else {
            Ok(GAME_MODE_IS_SINGLE)
        }
    }
}
pub trait GuessHashTrait {}

impl<T: Config> GuessHashTrait for Module<T> {}

pub trait GuessHashFunc<AccountId, GameIndex, ChipBalance>: GuessHashTrait {
    fn create_game(
        sender: &AccountId,
        bet_next_few_block: u32,
        amount: ChipBalance,
    ) -> sp_std::result::Result<GameIndex, DispatchError>;
    fn bet(
        sender: &AccountId,
        game_id: GameIndex,
        value: ChipBalance,
        game_mode: GameMode,
    ) -> dispatch::DispatchResult;
}
/// Provided to other modules（new game/ bet）
impl<T: Config> GuessHashFunc<T::AccountId, T::GameIndex, ChipBalance<T>> for Module<T> {
    fn create_game(
        sender: &T::AccountId,
        bet_next_few_block: u32,
        amount: ChipBalance<T>,
    ) -> sp_std::result::Result<T::GameIndex, DispatchError> {
        let game_id = Self::_create_game(sender, bet_next_few_block, amount)?;
        Ok(game_id)
    }

    fn bet(
        sender: &T::AccountId,
        game_id: T::GameIndex,
        value: ChipBalance<T>,
        game_mode: GameMode,
    ) -> dispatch::DispatchResult {
        Self::_bet(sender, game_id, value, game_mode)?;
        Ok(())
    }
}
