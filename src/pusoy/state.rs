use super::{Card, Cards, Play, PlayKind};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use serde_with::SerializeDisplay;
use std::{
    cmp::Ordering,
    fmt::{self, Display},
    ops,
};

#[derive(Debug)]
pub struct GameState {
    hands: SeatMap<Cards>,
    current_player: Seat,
    cards_on_table: Option<Play>,
    last_player_to_not_pass: Seat,
    winning_player: Option<Seat>,
}

impl GameState {
    pub fn new() -> GameState {
        let mut deck = Vec::from_iter(Cards::ENTIRE_DECK);
        deck.shuffle(&mut thread_rng());

        let mut hands = SeatMap::default();
        for ((_seat, hand), cards) in hands.iter_mut().zip(deck.chunks(13)) {
            *hand = cards.iter().cloned().collect::<Cards>();
        }

        let (current_player, _) = hands
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

    pub fn valid_plays(&self) -> Vec<Play> {
        Play::all(self.hands[self.current_player])
            .into_iter()
            .filter(|p| self.playable(p.cards).is_ok())
            .collect()
    }

    pub fn playable(&self, cards: Cards) -> Result<Play, PlayError> {
        if self.winning_player.is_some() {
            return Err(PlayError::AlreadyOver);
        }

        let play = Play::infer(cards).ok_or(PlayError::NonsenseCards)?;

        if !cards.is_subset(self.hands[self.current_player]) {
            return Err(PlayError::DontHaveCard);
        }

        if self.is_first_turn() {
            return if cards.contains(Card::THREE_OF_CLUBS) {
                Ok(play)
            } else {
                Err(PlayError::IsntPlayingThreeOfClubs)
            };
        }

        if self.has_control(self.current_player) {
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

        let hand = &mut self.hands[self.current_player];
        *hand = hand.without_all(cards);

        if self.hands[self.current_player].is_empty() {
            self.winning_player = Some(self.current_player);
        }

        if !play.is_pass() {
            self.last_player_to_not_pass = self.current_player;
            self.cards_on_table = Some(play);
        }

        self.current_player = self.current_player.next();

        Ok(play)
    }

    pub fn winning_player(&self) -> Option<Seat> {
        self.winning_player
    }

    pub fn played(&self, seat: Seat) -> bool {
        self.last_player_to_not_pass == seat
    }

    pub fn has_control(&self, seat: Seat) -> bool {
        self.current_player == seat && self.last_player_to_not_pass == seat
    }

    pub fn cards_on_table(&self) -> Option<Play> {
        self.cards_on_table
    }

    pub fn hand(&self, seat: Seat) -> Cards {
        self.hands[seat]
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
    AlreadyOver,
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
            Self::AlreadyOver => write!(f, "the game is already over"),
            Self::NonsenseCards => write!(f, "nonsense cards"),
            Self::DontHaveCard => write!(f, "don't have card to play"),
            Self::IsntPlayingThreeOfClubs => write!(f, "must play three of clubs"),
            Self::TooLow => write!(f, "play not high enough"),
            Self::WrongLength => write!(f, "wrong length"),
            Self::MustPlayOnControl => write!(f, "must play on control"),
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
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    pub fn next(self) -> Self {
        Self::from_i8(self as i8 + 1)
    }

    pub fn from_i8(index: i8) -> Self {
        match index.rem_euclid(4) {
            0 => Self::North,
            1 => Self::East,
            2 => Self::South,
            3 => Self::West,
            _ => unreachable!(),
        }
    }

    pub fn relative(self, relative: Relative) -> Self {
        Self::from_i8(relative as i8 + self as i8)
    }
}

impl Display for Seat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::North => write!(f, "north"),
            Self::East => write!(f, "east"),
            Self::South => write!(f, "south"),
            Self::West => write!(f, "west"),
        }
    }
}

#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct SeatMap<T> {
    inner: [T; Seat::ALL.len()],
}

impl<T> SeatMap<T> {
    pub fn iter(&self) -> impl Iterator<Item = (Seat, &'_ T)> {
        Seat::ALL.into_iter().zip(self.inner.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Seat, &'_ mut T)> {
        Seat::ALL.into_iter().zip(self.inner.iter_mut())
    }
}

impl<T> ops::Index<Seat> for SeatMap<T> {
    type Output = T;
    fn index(&self, seat: Seat) -> &Self::Output {
        &self.inner[seat as usize]
    }
}

impl<T> ops::IndexMut<Seat> for SeatMap<T> {
    fn index_mut(&mut self, seat: Seat) -> &mut Self::Output {
        &mut self.inner[seat as usize]
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Relative {
    My,
    Left,
    Across,
    Right,
}

impl Display for Relative {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::My => write!(f, "my"),
            Self::Left => write!(f, "left"),
            Self::Across => write!(f, "across"),
            Self::Right => write!(f, "right"),
        }
    }
}
