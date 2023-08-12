use super::{Play, all_plays, Card, Cards, PlayKind};
use rand::{thread_rng};
use std::iter::once;
use rand::seq::SliceRandom;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::cmp::Ordering;

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

        let hands: HashMap<_, _> = Seat::ALL.into_iter()
            .zip(deck.chunks(13))
            .map(|(seat, hand)| (seat, Cards::from_iter(hand.iter().cloned())))
            .collect();

        let (&current_player, _) = hands.iter()
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
        let pass = Play {
            cards: Cards::default(),
            kind: PlayKind::Pass,
            ranking_card: None,
        };
        once(pass)
            .chain(all_plays(self.my_hand()))
            .filter(|&p| self.can_play(p).is_ok())
            .collect()
    }
    
    // TODO: infer
//    pub fn can_play(&self, cards: Cards) -> Result<Play, GameError> {
    pub fn can_play(&self, play: Play) -> Result<(), GameError> {
        if !play.cards.is_subset(self.my_hand()) {
            return Err(GameError::DontHaveCard);
        }

        if self.is_first_turn() {
            if !play.cards.contains(Card::THREE_OF_CLUBS) {
                return Err(GameError::IsntPlayingThreeOfClubs);
            }
        } else if self.has_control() {
            // if we have control, we can pretty much do anything except passing
            if play.kind == PlayKind::Pass {
                return Err(GameError::MustPlayOnPass);
            }
        } else if play.kind != PlayKind::Pass {
            // here, we have our standard conditions, where we are not passing, and we don't have control

            // since we don't have control, we have to make sure they are making a valid play in the context
            // of the cards that they are trying to play on.
            let cards_down = self.cards_on_table.unwrap();

            // this is the problem
            if !play.same_kind(cards_down) {
                return Err(GameError::WrongLength);
            }

            let order = play.kind.cmp(&cards_down.kind)
                .then(play.ranking_card.unwrap().cmp(&cards_down.ranking_card.unwrap()));
            if order != Ordering::Greater {
                return Err(GameError::TooLow);
            }
        } 

        Ok(())
    }
    
    // TODO: infer
//    pub fn play(&mut self, cards: Cards) -> Result<Play, GameError> {
    pub fn play(&mut self, play: Play) -> Result<(), GameError> {
        // assumes that play is_legal
        self.can_play(play)?;

        self.hands
            .get_mut(&self.current_player).unwrap()
            .remove_all(play.cards);

        if self.hands[&self.current_player].is_empty() {
            self.winning_player = Some(self.current_player);
        }

        if play.kind != PlayKind::Pass {
            self.last_player_to_not_pass = self.current_player;
            self.cards_on_table = Some(play); 
        }

        self.current_player = self.current_player.next();

        Ok(())
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
