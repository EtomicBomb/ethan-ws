use serde::{Deserialize, Serialize};

use super::cards::{Card, Cards};
use std::fmt;

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Debug, Hash, Deserialize, Serialize)]
pub enum PlayKind {
    Pass,
    Single,
    Pair,

    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
}

impl fmt::Display for PlayKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            PlayKind::Pass => "pass",
            PlayKind::Single => "single",
            PlayKind::Pair => "pair",
            PlayKind::Straight => "straight",
            PlayKind::Flush => "flush",
            PlayKind::FullHouse => "full house",
            PlayKind::FourOfAKind => "four of a kind",
            PlayKind::StraightFlush => "straight flush",
        })
    }
}

impl PlayKind {
    fn len(self) -> usize {
        match self {
            PlayKind::Pass => 0,
            PlayKind::Single => 1,
            PlayKind::Pair => 2,
            PlayKind::Straight => 5,
            PlayKind::Flush => 5,
            PlayKind::FullHouse => 5,
            PlayKind::FourOfAKind => 5,
            PlayKind::StraightFlush => 5,
        }
    }

}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Deserialize, Serialize)]
pub struct Play {
    cards: Cards,
    kind: PlayKind,
    ranking_card: Option<Card>,
}

impl Play {
    fn pass() -> Play {
        Play {
            cards: Cards::empty(),
            kind: PlayKind::Pass,
            ranking_card: None,
        }
    }

    fn single(card: Card) -> Play {
        Play {
            cards: Cards::single(card),
            kind: PlayKind::Single,
            ranking_card: Some(card),
        }
    }

    fn pair(cards: Cards) -> Play {
        Play {
            cards,
            kind: PlayKind::Pair,
            ranking_card: cards.max_card(),
        }
    }

    fn straight(cards: Cards) -> Play {
        Play {
            cards,
            kind: PlayKind::Straight,
            ranking_card: cards.max_card(),
        }
    }

    fn flush(cards: Cards) -> Play {
        Play {
            cards,
            kind: PlayKind::Flush,
            ranking_card: cards.max_card(),
        }
    }

    fn full_house(three_of_a_kind: Cards, pair: Cards) -> Play {
        let mut cards = three_of_a_kind;
        cards.insert_all(pair);

        Play {
            cards,
            kind: PlayKind::FullHouse,
            ranking_card: three_of_a_kind.max_card(),
        }
    }

    fn four_of_a_kind(four_of_a_kind: Cards, trash_card: Card) -> Play {
        let mut cards = four_of_a_kind;
        cards.insert(trash_card);

        Play {
            cards,
            kind: PlayKind::FourOfAKind,
            ranking_card: four_of_a_kind.max_card(),
        }
    }

    fn straight_flush(straight: Play) -> Play {
        Play {
            cards: straight.cards,
            kind: PlayKind::StraightFlush,
            ranking_card: straight.ranking_card,
        }
    }

    pub fn is_pass(self) -> bool {
        self.kind == PlayKind::Pass
    }

    #[inline]
    pub fn len_eq(self, other: Play) -> bool {
        self.kind.len() == other.kind.len()
    }

    #[inline]
    pub fn can_play_on(self, other: Play) -> bool {
        if self.is_pass() { return true }

        if self.kind.len() != other.kind.len() {
            false
        } else if self.kind != other.kind {
            self.kind > other.kind
        } else {
            self.ranking_card.unwrap() > other.ranking_card.unwrap()
        }
    }

    pub fn kind(self) -> PlayKind {
        self.kind
    }

    #[inline]
    pub fn ranking_card(self) -> Option<Card> {
        self.ranking_card
    }

    #[inline]
    pub fn cards(self) -> Cards {
        self.cards
    }
}

pub struct AllPlays {
    pub straight_flushes: Vec<Play>,
    pub four_of_a_kind: Vec<Play>,
    pub full_houses: Vec<Play>,
    pub straights: Vec<Play>,
    pub flushes: Vec<Play>,
    pub pairs: Vec<Play>,
    pub singles: Vec<Play>,  
}

impl AllPlays {
    pub fn new(cards: Cards) -> AllPlays {
        let mut rank_blocks = RankBlocks::new(cards);

        AllPlays {
            straight_flushes: rank_blocks.straight_flushes(),
            four_of_a_kind: rank_blocks.four_of_a_kinds(),
            full_houses: rank_blocks.full_houses(),
            flushes: flushes(cards),
            straights: mem::take(&mut rank_blocks.straights),
            pairs: rank_blocks.pairs(),
            singles: singles(cards),
        }
    }
}

// TODO: since this entire module is based off of this one function, it should be refactored heavily
pub fn all_plays(cards: Cards) -> Vec<Play> {
    let mut plays = Vec::new();

    let mut rank_blocks = RankBlocks::new(cards);

    // five card hands
    plays.append(&mut rank_blocks.strait_flushes());
    plays.append(&mut rank_blocks.four_of_a_kinds());
    plays.append(&mut rank_blocks.full_houses());
    plays.append(&mut flushes(cards));
    plays.append(&mut rank_blocks.straits);

    // pairs
    plays.append(&mut rank_blocks.pairs());

    // singles
    plays.append(&mut singles(cards));

    plays
}

struct RankBlocks {
    cards: Cards,
    straights: Vec<Play>,
    blocks: [Cards; 13],
}

impl RankBlocks {
    fn new(cards: Cards) -> RankBlocks {
        fn straights(rank_blocks: [Cards; 13]) -> Vec<Play> {
            let mut straights = Vec::new();

            let mut blocks = Vec::with_capacity(5);

            for i in 0..13 {
                blocks.clear();
                blocks.extend(
                    (i .. i+5)
                        .map(|i| Vec::from_iter(rank_blocks[i % 13]))
                );

                straight_from_block(&blocks, &mut straights);
            }

            straights
        }
        
        let mut blocks: [Cards; 13] = Default::default();

        for card in cards.iter() {
            blocks[card.rank() as usize].insert(card);
        }

        RankBlocks {
            cards,
            blocks,
            straights: straights(blocks),
        }
    }

    fn n_of_a_kinds(&self, n: usize) -> Vec<Cards> {
        let mut chunks = Vec::new();

        for &block in self.blocks.iter() {
            if block.len() < n { continue } // this block is useless to us

            chunks.append(&mut permute(block, n));
        }

        chunks
    }

    fn pairs(&self) -> Vec<Play> {
        self.n_of_a_kinds(2).into_iter()
            .map(|cards| Play::pair(cards))
            .collect()
    }

    fn full_houses(&self) -> Vec<Play> {
        let mut full_houses = Vec::new();
        let pairs = self.n_of_a_kinds(2);

        for three_of_a_kind in self.n_of_a_kinds(3) {
            for &pair in pairs.iter() {
                if three_of_a_kind.disjoint(pair) {
                    full_houses.push(Play::full_house(three_of_a_kind, pair));
                }
            }
        }

        full_houses
    }

    fn four_of_a_kinds(&self) -> Vec<Play> {
        let mut four_of_a_kinds = Vec::new();

        for four_of_a_kind in self.n_of_a_kinds(4) {
            for trash_card in self.cards.iter() {
                if !four_of_a_kind.contains(trash_card) {
                    four_of_a_kinds.push(Play::four_of_a_kind(four_of_a_kind, trash_card));
                }
            }
        }

        four_of_a_kinds
    }


    fn straight_flushes(&self) -> Vec<Play> {
        self.straights.iter()
            .filter(|straight| straight.cards().all_same_suit())
            .map(|&straight| Play::straight_flush(straight))
            .collect()
    }
}


fn flushes(cards: Cards) -> Vec<Play> {
    // collect all of the cards
    let mut suit_blocks: [Cards; 4] = Default::default();

    for card in cards.iter() {
        suit_blocks[card.suit() as usize].insert(card);
    }

    suit_blocks.iter()
        .copied()
        .filter(|b| b.len() >= 5)
        .map(|block| {
            permute(block, 5).into_iter()
                .map(Play::flush)
        })
        .flatten()
        .collect()
}


fn straight_from_block(
    blocks: &[Vec<Card>],
    straights: &mut Vec<Play>,
) {
    let base: Vec<usize> = blocks.iter().map(|b| b.len()).collect();

    let f = |x: &[usize]| {
        let cards: Cards = blocks.iter().zip(x.iter())
            .map(|(block, &i)| block[i])
            .collect();

        straights.push(Play::straight(cards));
    };

    counter(&base, f);
}

fn permute(cards: Cards, len: usize) -> Vec<Cards> {
    let cards = Vec::from_iter(cards);
    permute_helper(&cards, len).into_iter()
        .map(|cards| cards.into_iter().collect())
        .collect()
}

fn permute_helper<T: Clone>(list: &[T], n: usize) -> Vec<Vec<T>> {
    assert!(list.len() >= n);
    let mut ret = Vec::new();

    if list.len() == n {
        ret.push(list.to_vec());
    } else if n == 1 {
        ret.extend(list.iter().map(|i| vec![i.clone()]));
    } else {
        for i in 0..=list.len() - n {
            let results = permute_helper(&list[i + 1..], n - 1);

            for mut r in results {
                r.insert(0, list[i].clone());
                ret.push(r);
            }
        }
    }

    ret
}

#[inline]
fn counter(base: &[usize], mut f: impl FnMut(&[usize])) {
    // a generalized version of counting in an arbitrary base
    // calls f on each number generated in the count
    // for example, counter(&[2, 2, 2], f) calls f on:
    //      &[0, 0, 0]
    //      &[1, 0, 0]
    //      &[0, 1, 0]
    //      &[1, 1, 0]
    //      etc.

    let len = base.len();

    let mut x = vec![0; len];

    let iter_count: usize = base.iter().product();

    for _ in 0..iter_count {
        f(&x);

        // try to "add one"
        for i in 0..len {
            if x[i] < base[i] - 1 {
                x[i] += 1;
                break;
            }

            x[i] = 0;
        }
    }
}
