use vec_map::{VecMap};

use std::f64::INFINITY;
use std::io::{self, BufReader};
use std::fs::File;
use std::collections::HashMap;


use rand::{thread_rng, Rng};
use rand::seq::SliceRandom;

use super::{Play, PlayKind, Card, GameState, Cards, all_plays, GameError};


pub fn choose_play(game: &GameState) -> Play {
    let game = SafeGameInterface { game };

    *game.valid_plays().choose(&mut thread_rng()).unwrap()

//    let my_hand = game.my_hand();
//    let potential_inserts = PotentialInserts::new(my_hand);
//    let depth_left = my_hand.len();
//
//    let mut memo = VecMap::with_capacity(MEMO_TABLE_CAPACITIES[depth_left-1]);
//    let cards_used_so_far = CardsUsedSoFar::new();
//
//    // these include some invalid plays
//    let all_plays_no_pass = all_plays(my_hand).into_iter()
//        .filter(|p| !p.is_pass())
//        .collect();
//
//    let result = cost_of_tail(all_plays_no_pass, depth_left, &potential_inserts, game, cards_used_so_far, &mut memo);
//
//    let desired_play = result.first_play();
//
//    match game.can_play(desired_play) {
//        Ok(_) => desired_play,
//        Err(_) => game.valid_plays()[0],
//    }
}

#[derive(Copy, Clone)]
struct SafeGameInterface<'a> {
    game: &'a GameState,
}

impl<'a> SafeGameInterface<'a> {
    fn my_hand(&self) -> Cards {
        self.game.my_hand()
    }
    
    fn cards_on_table(&self) -> Option<Play> {
        self.game.cards_on_table()
    }
    
    fn valid_plays(&self) -> Vec<Play> {
        self.game.valid_plays()
    }
    
    fn can_play(&self, play: Play) -> Result<(), GameError> {
        self.game.can_play(play)
    }
}

fn cost_of_tail(
    plays_available: Vec<Play>,
    depth_left: usize,
    potential_inserts: &PotentialInserts,
    game_interface: SafeGameInterface<'_>,
    cards_used_so_far: CardsUsedSoFar,
    memo: &mut VecMap<SearchState>,
) -> SearchState {

    if let Some(&state) = memo.get(cards_used_so_far.get_digest()) {
        return state; // we already have the best tail computed for this
    }

    if depth_left == 0 {
        // then, we have the tail and its cost; zero
        SearchState::new(game_interface)

    } else {
        let mut best_tail: Option<SearchState> = None;

        for &play in plays_available.iter() {
            let n_cards = play.cards().len();

            if depth_left < n_cards {
                continue; 
            }

            let plays_available_to_child: Vec<Play> = plays_available.iter()
                .filter(|p| p.cards().disjoint(play.cards()))
                .copied()
                .collect();

            let mut child_state_keeper = cards_used_so_far.clone();
            child_state_keeper.add_cards(play.cards(), potential_inserts);

            let mut result = cost_of_tail(plays_available_to_child, depth_left-n_cards, potential_inserts, game_interface, child_state_keeper, memo);

            result.add_play(play, game_interface);
            if best_tail.is_none() || result.total_cost < best_tail.as_ref().unwrap().total_cost {
                best_tail = Some(result);
            }
        }

        let ret = best_tail.unwrap();
        memo.insert(cards_used_so_far.get_digest(), ret.clone());
        ret
    }
}


// describes the state of the game after a move has been played
#[derive(Clone, Copy)]
struct SearchState {
    // this is the play that on our turn, we are looking to play on top of.
    // None if it is the first turn
    status: Status,
    total_cost: f64,
    first_play: Option<Play>,
}

#[derive(Clone, Copy)]
enum Status {
    FirstTurnOfGame,
    FirstAnalysis(Play), // previous term
    Rest(Play),          // four terms before
}

impl Status {
    fn is_first_turn(self) -> bool {
        match self {
            Status::Rest(_) => false,
            _ => true, 
        }
    }
}

impl<'a> SearchState {
    fn new(game_interface: SafeGameInterface<'_>) -> SearchState {
        let status = match game_interface.cards_on_table() {
            Some(play) => Status::FirstAnalysis(play.clone()),
            None => Status::FirstTurnOfGame,
        };

        SearchState {
            status,
            total_cost: 0.0,
            first_play: None,
        }
    }

    #[inline]
    fn add_play(&mut self, play: Play, game_interface: SafeGameInterface<'_>) {
        self.total_cost += match self.status {
            Status::FirstTurnOfGame => {
                if play.cards().contains(Card::THREE_OF_CLUBS) {
                    1.0 // we literally won't be able to pass
                } else {
                    INFINITY // this is always unplayable
                }
            }
            Status::FirstAnalysis(before) => {
                // we are trying to play directly on these cards
            
                if game_interface.can_play(play).is_ok() {
                    1.0
                } else {
                    // how many turns do we think it will take
                    get_expected_pass_count(before, play)
                }
            }
            Status::Rest(_four_turns_before) => {
                get_expected_pass_count(_four_turns_before, play)
            }
        };
        // change the status going forward
        if self.status.is_first_turn() {
            self.first_play = Some(play);
        }
        self.status = Status::Rest(play);
    }

    #[inline]
    fn first_play(self) -> Play {
        self.first_play.unwrap()
    }
}

struct PotentialInserts {
    map: [usize; 52],
}

impl PotentialInserts {
    fn new(cards: Cards) -> PotentialInserts {
        let mut map = [0; 52];
        for (i, card) in cards.iter().enumerate() {
            map[Self::index(card)] = i;
        }

        PotentialInserts { map }
    }
    
    fn index(card: Card) -> usize {
        4 * card.rank() as usize + card.suit() as usize
    }

    fn get_offset(&self, card: Card) -> usize {
        self.map[Self::index(card)]
    }
}

// we cannot just keep a u64 and store all of the cards because then get_digest wont fit into the VecMap
#[derive(Clone)]
struct CardsUsedSoFar {
    seen_so_far: u16, // only  use lower 13 bits
}

impl CardsUsedSoFar {
    fn new() -> CardsUsedSoFar {
        CardsUsedSoFar {
            seen_so_far: 0, // empty
        }
    }

    fn add_card(&mut self, card: Card, potential_inserts: &PotentialInserts) {
        let offset = potential_inserts.get_offset(card);
        self.seen_so_far |= 1 << offset;
    }

    fn add_cards(&mut self, cards: Cards, potential_inserts: &PotentialInserts) {
        for card in cards.iter() {
            self.add_card(card, potential_inserts);
        }
    }

    fn get_digest(&self) -> usize {
        self.seen_so_far as usize
    }
}

fn get_expected_pass_count(_play1: Play, _play2: Play) -> f64 {
    3.0
}

// determined experimentally
const MEMO_TABLE_CAPACITIES: [usize; 13] = [
    1,
    14,
    92,
    378,
    1093,
    2380,
    4096,
    5812,
    7099,
    7814,
    8100,
    8178,
    8191,
];
