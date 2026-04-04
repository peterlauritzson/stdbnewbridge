import { CLUBS, DIAMONDS, HEARTS, SPADES, type Card } from '../types';

const SUIT_SYMBOLS: Record<number, string> = {
  [CLUBS]: '♣',
  [DIAMONDS]: '♦',
  [HEARTS]: '♥',
  [SPADES]: '♠',
};

const SUIT_NAMES: Record<number, string> = {
  [CLUBS]: 'Clubs',
  [DIAMONDS]: 'Diamonds',
  [HEARTS]: 'Hearts',
  [SPADES]: 'Spades',
};

const SUIT_COLORS: Record<number, string> = {
  [CLUBS]: '#1a1a2e',
  [DIAMONDS]: '#e94560',
  [HEARTS]: '#e94560',
  [SPADES]: '#1a1a2e',
};

const RANK_NAMES: Record<number, string> = {
  2: '2', 3: '3', 4: '4', 5: '5', 6: '6', 7: '7', 8: '8',
  9: '9', 10: '10', 11: 'J', 12: 'Q', 13: 'K', 14: 'A',
};

export function suitSymbol(suit: number): string {
  return SUIT_SYMBOLS[suit] ?? '?';
}

export function suitName(suit: number): string {
  return SUIT_NAMES[suit] ?? '?';
}

export function suitColor(suit: number): string {
  return SUIT_COLORS[suit] ?? '#333';
}

export function rankName(rank: number): string {
  return RANK_NAMES[rank] ?? String(rank);
}

export function cardLabel(card: Card): string {
  return `${rankName(card.rank)}${suitSymbol(card.suit)}`;
}

/** Sort cards by suit (spades first) then rank descending within each suit */
export function sortCards(cards: Card[]): Card[] {
  return [...cards].sort((a, b) => {
    // Suit order: Spades, Hearts, Diamonds, Clubs (descending suit value)
    if (a.suit !== b.suit) return b.suit - a.suit;
    // Higher rank first within same suit
    return b.rank - a.rank;
  });
}

/** Group cards by suit, sorted in display order */
export function groupBySuit(cards: Card[]): Map<number, Card[]> {
  const groups = new Map<number, Card[]>();
  for (const suit of [SPADES, HEARTS, DIAMONDS, CLUBS]) {
    const suitCards = cards.filter(c => c.suit === suit).sort((a, b) => b.rank - a.rank);
    if (suitCards.length > 0) {
      groups.set(suit, suitCards);
    }
  }
  return groups;
}

export function trumpName(suit: number | null): string {
  if (suit === null) return 'NT';
  return suitSymbol(suit);
}
