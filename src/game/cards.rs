use serde_with::{DeserializeFromStr, SerializeDisplay};
use serde::de::{Deserialize, Deserializer, Visitor, SeqAccess};
use serde::ser::{Serialize, Serializer, SerializeSeq};
use std::fmt;
use std::str::FromStr;
use std::iter::{FromIterator};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Cards {
    bits: u64, // in suit-major order, with 3♣ as the least significant bit 
}

impl Cards {
    pub const CLUBS: Self = Self { bits: 0x1111111111111 };
    pub const SPADES: Self = Self { bits: 0x2222222222222 };
    pub const HEARTS: Self = Self { bits: 0x4444444444444 }; 
    pub const DIAMONDS: Self = Self { bits: 0x8888888888888 }; 
    pub const ENTIRE_DECK: Cards = Self { bits: 0xfffffffffffff };
    pub const SUITS: [Self; 4] = [Self::CLUBS, Self::SPADES, Self::HEARTS, Self::DIAMONDS];
    
    pub const fn single(card: Card) -> Self {
        Self { bits: 1 << card.index }
    }

    pub fn copy_rank(card: Card) -> Self {
        let rank = card.rank();
        let bits = 0xf << (4 * rank);
        Cards { bits }
    }

    pub fn insert(&mut self, card: Card) {
        self.insert_all(Self::single(card))
    }

    pub fn insert_all(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    pub fn remove(&mut self, card: Card) {
        self.remove_all(Self::single(card));
    }

    pub fn remove_all(&mut self, other: Self) {
        self.bits &= !other.bits;
    }

    pub fn contains(self, card: Card) -> bool {
        Self::single(card).is_subset(self)
    }

    pub fn is_subset(self, other: Self) -> bool {
        self.bits & !other.bits == 0
    }

    pub fn disjoint(self, other: Self) -> bool {
        self.bits & other.bits == 0 
    }

    pub fn is_empty(self) -> bool {
        self.bits == 0
    }

    pub fn intersection(self, other: Self) -> Self {
        let bits = self.bits & other.bits;
        Cards { bits }
    }

    pub fn len(self) -> usize {
        self.bits.count_ones() as usize
    }

    pub fn all_same_rank(self) -> bool {
        // TODO: why not self.bits.trailing_zeros() & !3
        let after = (self.bits.trailing_zeros() | 3) - 3; // round down to multiple of 4
        let rank_cluster = self.bits >> after; // move our rank cluster to the lower 4 bits
        rank_cluster < 16 // if they're all the same rank, then this should be the only rank cluster
    }

    pub fn all_same_suit(self) -> bool {
        Self::SUITS.into_iter().any(|s| self.is_subset(s))
    }

    pub fn max_card(self) -> Option<Card> {
        match self.bits.leading_zeros() {
            64 => None,
            n => Some(Card { index: 63 - n as u8 }),
        }
    }

    pub fn min_card(self) -> Option<Card> {
        match self.bits.trailing_zeros() {
            64 => None,
            n => Some(Card { index: n as u8 })
        }
    }

    pub fn iter(self) -> CardsIter {
        CardsIter { cards: self }
    }
}

impl fmt::Debug for Cards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(*self).finish()
    }
}

impl Extend<Card> for Cards {
    fn extend<I: IntoIterator<Item=Card>>(&mut self, iter: I) {
        for card in iter.into_iter() {
            self.insert(card);
        }
    }
}

impl FromIterator<Card> for Cards {
    fn from_iter<I: IntoIterator<Item=Card>>(iter: I) -> Self {
        let mut cards = Cards::default();
        cards.extend(iter);
        cards
    }
}

impl IntoIterator for Cards {
    type Item = Card;
    type IntoIter = CardsIter;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'de> Deserialize<'de> for Cards {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CardsVisitor;

        impl<'de> Visitor<'de> for CardsVisitor {
            type Value = Cards;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "list of cards")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Cards, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let mut cards = Cards::default();
                while let Some(card) = seq.next_element()? {
                    cards.insert(card);
                }
                Ok(cards)
            }
        }

        deserializer.deserialize_seq(CardsVisitor)
    }
}

impl Serialize for Cards {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for card in self.iter() {
            seq.serialize_element(&card)?;
        }
        seq.end()
    }
}

pub struct CardsIter {
    cards: Cards,
}

impl Iterator for CardsIter {
    type Item = Card;
    fn next(&mut self) -> Option<Self::Item> {
        let card = self.cards.min_card()?;
        self.cards.remove(card);
        Some(card)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, SerializeDisplay, DeserializeFromStr)]
pub struct Card {
    index: u8,
}

impl Card {
    pub const THREE_OF_CLUBS: Self = Self { index: 0 };

    pub fn rank(self) -> u8 {
        self.index / 4
    }

    pub fn suit(self) -> u8 {
        self.index % 4
    }
}

const RANKS: [&str; 13] = ["3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K", "A", "2"];
const SUITS: [&str; 4] = ["♣", "♠", "♥", "♦"];

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank = RANKS[self.rank() as usize];
        let suit = SUITS[self.suit() as usize];
        write!(f, "{}{}", rank, suit)
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug)]
pub struct ToCardError; 

impl fmt::Display for ToCardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to convert to card")
    }
}

impl FromStr for Card {
    type Err = ToCardError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rank = s.get(0..1).ok_or(ToCardError)?;
        let rank = RANKS.into_iter().position(|r| rank == r).ok_or(ToCardError)? as u8;
        let suit = s.get(1..).ok_or(ToCardError)?;
        let suit = SUITS.into_iter().position(|s| suit == s).ok_or(ToCardError)? as u8;
        Ok(Card { index: rank * 4 + suit })
    }
}

#[cfg(test)]
mod tests {
    use super::{Card, Cards};
    use std::collections::HashSet;
    use serde_json;
    use std::iter::once;

    #[test]
    fn cards_set_len() {
        assert_eq!(Cards::default().len(), 0);
        assert_eq!(Cards::default().iter().count(), 0);
        assert_eq!(Cards::default().into_iter().count(), 0);

        assert_eq!(Cards::ENTIRE_DECK.len(), 52);
        assert_eq!(Cards::ENTIRE_DECK.iter().count(), 52);
        assert_eq!(Cards::ENTIRE_DECK.into_iter().count(), 52);

        for suit in Cards::SUITS {
            assert_eq!(suit.len(), 13);
            assert_eq!(suit.iter().count(), 13);
            assert_eq!(suit.into_iter().count(), 13);
        }

        let cards_set: HashSet<_> = HashSet::from_iter(Cards::ENTIRE_DECK);
        assert_eq!(cards_set.len(), 52);
    }

    #[test]
    fn rank_suit_unique() {
        let mut things: HashSet<(u8, u8)> = HashSet::new();
        for card in Cards::ENTIRE_DECK {
            things.insert((card.rank(), card.suit()));
        }
        assert_eq!(things.len(), 52);
    }

    #[test]
    fn encoding() {
        let mut cards = Cards::ENTIRE_DECK.into_iter();
        let three_clubs = cards.next().unwrap();
        assert_eq!(three_clubs, Card::THREE_OF_CLUBS);
        let s = format!("{}", three_clubs);
        assert_eq!(s, "3♣");
        let s = serde_json::to_string(&three_clubs).unwrap();
        assert_eq!(s, "\"3♣\"");

        let three_spades = cards.next().unwrap();
        let s = format!("{}", three_spades);
        assert_eq!(s, "3♠");
        let s = serde_json::to_string(&three_spades).unwrap();
        assert_eq!(s, "\"3♠\"");
    }


    #[test]
    fn cards_encoding_round_trip() {
        let cardss = once(Cards::ENTIRE_DECK)
            .chain(Cards::SUITS)
            .chain(Cards::ENTIRE_DECK.into_iter().map(Cards::single));
        for cards in cardss {
            let s = serde_json::to_string(&cards).unwrap();
            let c = serde_json::from_str(&s).unwrap();
            assert_eq!(cards, c);
        }
    }

    #[test]
    fn card_encoding_round_trip() {
        for card in Cards::ENTIRE_DECK {
            let s = format!("{card}");
            let c = s.parse::<Card>().expect("could not parse");
            assert_eq!(card, c);

            let s = serde_json::to_string(&card).unwrap();
            let c = serde_json::from_str(&s).unwrap();
            assert_eq!(card, c);
        }
    }

}
