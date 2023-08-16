use super::{Card, Cards, Play, PlayKind};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use serde_with::SerializeDisplay;

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
        let mut deck = Vec::from_iter(Cards::ENTIRE_DECK);
        deck.shuffle(&mut thread_rng());

        let hands: HashMap<_, _> = Seat::ALL
            .into_iter()
            .zip(deck.chunks(13))
            .map(|(seat, hand)| (seat, Cards::from_iter(hand.iter().cloned())))
            .collect();

        let (&current_player, _) = hands
            .iter()
            .find(|(_, hand)| hand.contains(Card::THREE_OF_CLUBS))
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
        Play::all(self.my_hand())
            .into_iter()
            .filter(|p| self.playable(p.cards).is_ok())
            .collect()
    }

    pub fn playable(&self, cards: Cards) -> Result<Play, PlayError> {
        let play = Play::infer(cards).ok_or(PlayError::NonsenseCards)?;

        if !cards.is_subset(self.my_hand()) {
            return Err(PlayError::DontHaveCard);
        }

        if self.is_first_turn() {
            return if cards.contains(Card::THREE_OF_CLUBS) {
                Ok(play)
            } else {
                Err(PlayError::IsntPlayingThreeOfClubs)
            };
        }

        if self.has_control() {
            return if play.is_pass() {
                Err(PlayError::MustPlayOnControl)
            } else {
                Ok(play)
            };
        }

        let cards_down = self.cards_on_table.unwrap();

        let order = match (play.kind, cards_down.kind) {
            (PlayKind::Pass, _) => Ordering::Greater,
            (PlayKind::Single(this), PlayKind::Single(other)) => this.cmp(&other),
            (PlayKind::Pair(this), PlayKind::Pair(other)) => this.cmp(&other),
            (PlayKind::Poker(this), PlayKind::Poker(other)) => this.cmp(&other),
            _ => return Err(PlayError::WrongLength),
        };

        if order != Ordering::Greater {
            return Err(PlayError::TooLow);
        }

        Ok(play)
    }

    pub fn play(&mut self, cards: Cards) -> Result<Play, PlayError> {
        let play = self.playable(cards)?;

        let current_hand = self.hands.get_mut(&self.current_player).unwrap();
        *current_hand = current_hand.remove_all(cards);

        if self.hands[&self.current_player].is_empty() {
            self.winning_player = Some(self.current_player);
        }

        if !play.is_pass() {
            self.last_player_to_not_pass = self.current_player;
            self.cards_on_table = Some(play);
        }

        self.current_player = self.current_player.next();

        Ok(play)
    }

    pub fn has_control(&self) -> bool {
        self.last_player_to_not_pass == self.current_player
    }

    pub fn cards_on_table(&self) -> Option<Play> {
        self.cards_on_table
    }

    pub fn hand(&self, seat: Seat) -> Cards {
        self.hands[&seat]
    }

    pub fn my_hand(&self) -> Cards {
        self.hand(self.current_player)
    }

    pub fn current_player(&self) -> Seat {
        self.current_player
    }

    pub fn is_first_turn(&self) -> bool {
        self.cards_on_table.is_none()
    }
}

#[derive(SerializeDisplay, Debug)]
pub enum PlayError {
    NonsenseCards,
    DontHaveCard,
    IsntPlayingThreeOfClubs,
    TooLow,
    WrongLength,
    MustPlayOnControl,
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlayError::NonsenseCards => write!(f, "nonsense cards"),
            PlayError::DontHaveCard => write!(f, "don't have card to play"),
            PlayError::IsntPlayingThreeOfClubs => write!(f, "must play three of clubs"),
            PlayError::TooLow => write!(f, "play not high enough"),
            PlayError::WrongLength => write!(f, "wrong length"),
            PlayError::MustPlayOnControl => write!(f, "must play on control"),
        }
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
