use rand::seq::SliceRandom;
use rand::thread_rng;

use super::{Cards, GameState, Play, PlayError};

pub fn choose_play(game: &GameState) -> Play {
    let game = SafeGameInterface { game };
    *game.valid_plays().choose(&mut thread_rng()).unwrap()
}

#[derive(Copy, Clone)]
struct SafeGameInterface<'a> {
    game: &'a GameState,
}

impl<'a> SafeGameInterface<'a> {

//    fn my_hand(&self) -> Cards {
//        self.game.my_hand()
//    }

//    fn cards_on_table(&self) -> Option<Play> {
//        self.game.cards_on_table()
//    }

    fn valid_plays(&self) -> Vec<Play> {
        self.game.valid_plays()
    }

//    fn playable(&self, cards: Cards) -> Result<Play, PlayError> {
//        self.game.playable(cards)
//    }
//
}
