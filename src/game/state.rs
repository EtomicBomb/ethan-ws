use super::plays::Play;
use super::plays::all_plays;
use crate::cards::{Card, Cards};
use rand::{thread_rng, Rng};
use std::iter::once;
use rand::SliceRandom;

#[derive(Debug)]
pub struct GameState {
    hands: HashMap<Seat, Cards>,
    current_player: Seat,
    cards_on_table: Option<Play>,
    last_player_to_not_pass: Seat,
    winning_player: Option<Seat>,
}

impl Default for GameState {
    fn default() -> GameState {
        let mut deck = Vec::new(Cards::ENTIRE_DECK);
        deck.shuffle(&mut thread_rng());

        let hands = deck.chunks(13)
            .zip(Seat::ALL)
            .map(|(seat, hand)| (seat, Cards::from_iter(hand.iter().cloned()))
            .collect();

        let (current_player, _) = hands.iter()
            .find(|(seat, hand)| hand.contains(Card::THREE_OF_CLUBS))
            .unwrap();

        GameState {
            hands,
            current_player,
            cards_on_table: None,
            last_player_to_not_pass: current_player,
            winning_player: None,
        }
    }
}

impl GameState {
    pub fn valid_plays(&self) -> Vec<Play> {
        once(Play::pass())
            .chain(all_plays(self.my_hand()))
            .filter(|&p| self.can_play(p).is_ok())
            .collect()
    }
    
    pub fn can_play(&self, play: Play) -> Result<(), GameError> {
        if !play.cards().is_subset(self.my_hand()) {
            return Err(GameError::DontHaveCard);
        }

        if self.is_first_turn() {
            if !play.cards().contains(Card::THREE_OF_CLUBS) {
                return Err(GameError::IsntPlayingThreeOfClubs);
            }
        } else if self.has_control() {
            // if we have control, we can pretty much do anything except passing
            if play.is_pass() {
                return Err(GameError::MustPlayOnPass);
            }
        } else if !play.is_pass() {
            // here, we have our standard conditions, where we are not passing, and we don't have control

            // since we don't have control, we have to make sure they are making a valid play in the context
            // of the cards that they are trying to play on.
            let cards_down = self.cards_on_table.unwrap();

            // this is the problem
            if !play.len_eq(cards_down) {
                return Err(GameError::WrongLength);
            }

            if !play.can_play_on(cards_down) {
                return Err(GameError::TooLow);
            }
        } 

        Ok(())
    }

    pub fn play(&mut self, play: Play) -> Result<(), GameError> {
        // assumes that play is_legal
        self.can_play(play)?;

        self.hands[&self.current_player].remove_all(play.cards());

        if self.hands[&self.current_player].is_empty() {
            self.winning_player = Some(self.current_player);
        }

        if !play.is_pass() {
            self.last_player_to_not_pass = self.current_player;
            self.cards_on_table = Some(play); 
        }

        self.current_player = self.current_player.next();

        Ok(())
    }

    pub fn has_control(&self) -> bool {
        self.last_player_to_not_pass == self.current_player
    }

    pub fn winning_player(&self) -> Option<Seat> {
        self.winning_player
    }

    pub fn cards_on_table(&self) -> Option<Play> {
        self.cards_on_table
    }

    pub fn hand(&self, seat: Seat) -> Cards {
        self.hands[&self.current_player]
    }

    pub fn my_hand(&self) -> Cards {
        self.hand(self.current_player)
    }

    pub fn current_player(&self) -> Seat { self.current_player }

    pub fn is_first_turn(&self) -> bool {
        self.cards_on_table.is_none()
    }
}

#[derive(Debug)]
pub enum GameError {
    DontHaveCard,
    IsntPlayingThreeOfClubs,
    TooLow,
    WrongLength,
    MustPlayOnPass,
    PlayDoesntExist,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Seat {
    North,
    East,
    South,
    West,
}

impl Seat {
    pub const ALL: [Seat; 4] = [Seat::North, Seat::East, Seat::South, Seat::West];

    pub fn next(self) -> Seat {
        match self {
            Seat::North => Seat::East,
            Seat::East => Seat::South,
            Seat::South => Seat::West,
            Seat::West => Seat::North,
        }
    }
}
