import { useState } from 'react';
import type { Card } from '../types';
import { groupBySuit, suitSymbol, suitColor } from '../lib/cardUtils';
import { CardView } from './CardView';

interface PlayerHandProps {
  cards: Card[];
  isMyHand: boolean;
  playable?: boolean;
  ledSuit?: number | null;
  isLeader?: boolean;
  onPlay?: (cardIds: number[]) => void;
  onPass?: () => void;
}

export function PlayerHand({ cards, isMyHand, playable, ledSuit, isLeader, onPlay, onPass }: PlayerHandProps) {
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const grouped = groupBySuit(cards);

  // Determine which suit we must follow (if any)
  const mustFollowSuit = ledSuit != null && cards.some(c => c.suit === ledSuit)
    ? ledSuit
    : null;

  const isCardPlayable = (card: Card) => {
    if (!playable) return false;
    // Must follow suit if we have cards in that suit
    if (mustFollowSuit != null && card.suit !== mustFollowSuit) return false;
    return true;
  };

  const toggleCard = (cardId: number) => {
    if (!isMyHand || !playable) return;
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(cardId)) {
        next.delete(cardId);
      } else {
        const card = cards.find(c => c.cardId === cardId);
        if (!card || !isCardPlayable(card)) return prev;
        
        // Only allow selecting cards of the same suit
        const existingCard = cards.find(c => next.has(c.cardId));
        if (existingCard && existingCard.suit !== card.suit) {
          next.clear();
        }
        
        next.add(cardId);
      }
      return next;
    });
  };

  const handlePlay = () => {
    if (selectedIds.size === 0) return;
    onPlay?.([...selectedIds]);
    setSelectedIds(new Set());
  };

  const handlePass = () => {
    onPass?.();
    setSelectedIds(new Set());
  };

  if (!isMyHand) {
    // Show card backs for opponents
    return (
      <div className="hand hand-opponent">
        {cards.map((_, i) => (
          <CardView key={i} faceDown small />
        ))}
        <span className="hand-count">{cards.length}</span>
      </div>
    );
  }

  return (
    <div className="hand hand-mine">
      <div className="hand-cards">
        {[...grouped.entries()].map(([suit, suitCards]) => (
          <div key={suit} className="suit-group">
            <span className="suit-label" style={{ color: suitColor(suit) }}>
              {suitSymbol(suit)}
            </span>
            {suitCards.map(card => {
              const canPlay = isCardPlayable(card);
              return (
                <CardView
                  key={card.cardId}
                  card={card}
                  selected={selectedIds.has(card.cardId)}
                  playable={canPlay}
                  onClick={() => toggleCard(card.cardId)}
                />
              );
            })}
          </div>
        ))}
      </div>

      {playable && (
        <div className="hand-actions">
          <button className="btn btn-primary" onClick={handlePlay} disabled={selectedIds.size === 0}>
            Play {selectedIds.size > 0 ? `(${selectedIds.size})` : ''}
          </button>
          {!isLeader && (
            <button className="btn btn-secondary" onClick={handlePass}>
              Pass
            </button>
          )}
        </div>
      )}
    </div>
  );
}
