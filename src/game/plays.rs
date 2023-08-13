use serde::{Deserialize, Serialize};
use super::{Card, Cards};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Deserialize, Serialize)]
pub struct Play {
    pub cards: Cards,
    pub kind: PlayKind,
}

impl Play {
    pub fn infer(cards: Cards) -> Option<Self> {
        let kind = PlayKind::infer(cards)?;
        Some(Play { cards, kind })
    }

    pub fn is_pass(self) -> bool {
        matches!(self.kind, PlayKind::Pass)
    }

    pub fn all(cards: Cards) -> Vec<Self> {
        let rank_blocks = RankBlocks::new(cards);
        let straights = rank_blocks.straights();

        let mut plays = Vec::new();
        plays.append(&mut rank_blocks.singles());
        plays.append(&mut rank_blocks.pairs());
        plays.extend(straights.iter().cloned());
        plays.append(&mut flushes(cards));
        plays.append(&mut rank_blocks.full_houses());
        plays.append(&mut rank_blocks.quadruples());
        plays.append(&mut straight_flushes(&straights));

        plays
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlayKind {
    Pass,
    Single(Card),
    Pair(Card),
    Poker(Poker),
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Debug, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Poker {
    Straight(Card),
    Flush(Card),
    FullHouse(Card),
    Quadruple(Card),
    StraightFlush(Card),
}

impl PlayKind {
    pub fn infer(cards: Cards) -> Option<Self> {
        if cards.len() == 0 {
            return Some(PlayKind::Pass);
        }

        {
            let ranking_card = cards.max().unwrap();
            if cards.len() == 1 {
                return Some(PlayKind::Single(ranking_card));
            }

            if cards.len() == 2 && cards.all_same_rank() {
                return Some(PlayKind::Pair(ranking_card));
            }
            
            match (cards.len(), is_straight(cards), cards.all_same_suit()) {
                (5, true, true) => return Some(PlayKind::Poker(Poker::StraightFlush(ranking_card))),
                (5, true, false) => return Some(PlayKind::Poker(Poker::Straight(ranking_card))),
                (5, false, true) => return Some(PlayKind::Poker(Poker::Flush(ranking_card))),
                _ => {},
            }
        }

        {
            let mut xs = [
                cards.min().unwrap(),
                cards.max().unwrap(),
            ].map(|x| Cards::copy_rank(x).intersection(cards));
            xs.sort_by_key(|x| x.len());
            let [a, b] = xs;
            let ranking_card = b.max().unwrap();
            match (a.len(), b.len()) {
                (2, 3) => return Some(PlayKind::Poker(Poker::FullHouse(ranking_card))),
                (1, 4) => return Some(PlayKind::Poker(Poker::Quadruple(ranking_card))),
                _ => {},
            }
        }

        None
    }
}


struct RankBlocks {
    cards: Cards,
    blocks: [Block; 13],
}

impl RankBlocks {
    fn new(cards: Cards) -> RankBlocks {
        let mut blocks: [Block; 13] = Default::default();

        for card in cards {
            blocks[card.rank() as usize].insert(card);
        }

        RankBlocks { cards, blocks }
    }

    fn singles(&self) -> Vec<Play> {
        self.cards.into_iter()
            .map(|card| Play { 
                cards: Cards::single(card), 
                kind: PlayKind::Single(card), 
            })
            .collect() 
    }

    fn pairs(&self) -> Vec<Play> {
        self.blocks.iter()
            .flat_map(|block| block.pairs.iter().map(|&cards| {
                let ranking_card = cards.max().unwrap();
                let kind = PlayKind::Pair(ranking_card);
                Play { cards, kind }
            }))
        .collect()
    }

    fn straights(&self) -> Vec<Play> {
        let mut ret = Vec::new();

        for i in 0..13 {
            let blocks = Vec::from_iter(
                (i..i+5).map(|i| &self.blocks[i%13].singles)
            );

            let base = Vec::from_iter(blocks.iter().map(|b| b.len()));

            counter(&base, |x| {
                let cards: Cards = blocks.iter().zip(x.iter())
                    .map(|(block, &i)| block[i])
                    .collect();

                ret.push(Play {
                    cards,
                    kind: PlayKind::Poker(Poker::Straight(cards.max().unwrap())),
                });
            });
        }

        ret
    }

    fn full_houses(&self) -> Vec<Play> {
        fn helper(block1: &Block, block2: &Block, ret: &mut Vec<Play>) {
            for &triple in block2.triples.iter() {
                let ranking_card = triple.max().unwrap();
                let kind = PlayKind::Poker(Poker::FullHouse(ranking_card));

                for &pair in block1.pairs.iter() {
                    let cards = pair.insert_all(triple);
                    ret.push(Play { cards, kind });
                }
            }
        }

        let mut ret = Vec::new();
        for i in 0..self.blocks.len() {
            for j in i+1..self.blocks.len() {
                helper(&self.blocks[i], &self.blocks[j], &mut ret);
                helper(&self.blocks[j], &self.blocks[i], &mut ret);
            }
        }
        ret
    }

    fn quadruples(&self) -> Vec<Play> {
        fn helper(block1: &Block, block2: &Block, ret: &mut Vec<Play>) {
            let Some(quadruple) = block1.quadruples else { return };
            let ranking_card = quadruple.max().unwrap();
            let kind = PlayKind::Poker(Poker::Quadruple(ranking_card));

            for &single in block2.singles.iter() {
                let cards = quadruple.insert(single);
                ret.push(Play { cards, kind });
            }
        }

        let mut ret = Vec::new();
        for i in 0..self.blocks.len() {
            for j in i+1..self.blocks.len() {
                helper(&self.blocks[i], &self.blocks[j], &mut ret);
                helper(&self.blocks[j], &self.blocks[i], &mut ret);
            }
        }
        ret
    }
}

#[derive(Default)]
struct Block {
    singles: Vec<Card>,
    pairs: Vec<Cards>,
    triples: Vec<Cards>,
    quadruples: Option<Cards>,
}

impl Block {
    fn insert(&mut self, card: Card) {
        if let Some(&triple) = self.triples.first() {
            self.quadruples = Some(triple.insert(card));
        }

        for &pair in self.pairs.iter() {
            self.triples.push(pair.insert(card));
        }

        for &old in self.singles.iter() {
            let pair = Cards::single(old).insert(card);
            self.pairs.push(pair);
        }

        self.singles.push(card);
    }
}

fn flushes(cards: Cards) -> Vec<Play> {
    fn helper<F: FnMut(Cards)>(suit: Cards, mut visit: F) {
        let mut remaining = suit.len();

        let mut currents = Vec::from([Cards::default()]);
        let mut add_to_currents = Vec::new();
        for card in suit {
            for current in currents.iter().cloned() {
                let current = current.insert(card);

                if current.len() == 5 {
                    visit(current);
                } else if current.len() + remaining >= 5 {
                    add_to_currents.push(current);
                }
            }

            remaining -= 1;
            currents.append(&mut add_to_currents);
        }
    }

    let mut ret = Vec::new();
    for suit in Cards::SUITS {
        let suit = suit.intersection(cards);
        helper(suit, |flush| {
            let ranking_card = flush.max().unwrap();
            ret.push(Play {
                cards: flush,
                kind: PlayKind::Poker(Poker::Flush(ranking_card)),
            });
        });
    }
    ret
}

fn straight_flushes(straights: &[Play]) -> Vec<Play> {
    straights.iter()
        .filter(|straight| straight.cards.all_same_suit())
        .map(|&straight| Play {
            cards: straight.cards,
            kind: PlayKind::Poker(Poker::StraightFlush(straight.cards.max().unwrap())),
        })
        .collect()
}

fn is_straight(cards: Cards) -> bool {
    let straights_start = 11; // the contiguous block of valid straights starts at aces
    let num_straights = 10; // there are 10 kinds of straights (starting or ending at ace)
    (straights_start..straights_start+num_straights).any(|i| {
        (i..i+5).map(|j| Cards::with_rank(j%13)).all(|rank| !rank.disjoint(cards))
    })
}

fn counter<F: FnMut(&'_ [usize])>(base: &[usize], mut visit: F) {
    if base.iter().any(|&b| b == 0) { return }
    let len = base.len();
    let mut x = vec![0; len];

    loop {
        visit(&x);

        // try to "add one" to x
        let mut i = 0;
        while i < len {
            if x[i] < base[i]-1 { break }
            x[i] = 0;
            i += 1;
        }
        if i == len { break }
        x[i] += 1;
    }
}
