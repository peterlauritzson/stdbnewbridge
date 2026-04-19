import { useState, useMemo, useCallback } from 'react';
import type { Card, Trick, TrickPlay } from '../types';
import { SEAT_NAMES, SPADES, HEARTS, DIAMONDS, CLUBS } from '../types';
import { suitSymbol, suitColorBright, rankName } from '../lib/cardUtils';
import { CardView } from './CardView';

interface HandReplayProps {
  allCards: Card[];
  tricks: Trick[];
  trickPlays: TrickPlay[];
  seatLabels: Record<number, string>;
  relativeSeats: { top: number; left: number; right: number; bottom: number };
  trump: number | null;
  onClose: () => void;
}

interface ReplayState {
  trickIndex: number;  // which trick (0-based)
  playIndex: number;   // how many plays to show within the trick (-1 = none, 0..3)
}

export function HandReplay({
  allCards,
  tricks,
  trickPlays,
  seatLabels,
  relativeSeats,
  trump,
  onClose,
}: HandReplayProps) {
  const sortedTricks = useMemo(
    () => [...tricks].sort((a, b) => a.trickNumber - b.trickNumber),
    [tricks]
  );

  const playsPerTrick = useMemo(() => {
    const map = new Map<number, TrickPlay[]>();
    for (const t of sortedTricks) {
      const plays = trickPlays
        .filter(tp => tp.trickId === t.trickId)
        .sort((a, b) => a.sequence - b.sequence);
      map.set(t.trickId, plays);
    }
    return map;
  }, [sortedTricks, trickPlays]);

  // Track all card ids played up to each point
  const totalPlays = useMemo(() => {
    let count = 0;
    for (const t of sortedTricks) {
      const plays = playsPerTrick.get(t.trickId) ?? [];
      count += plays.length;
    }
    return count;
  }, [sortedTricks, playsPerTrick]);

  // Use a single step counter (0 = no plays shown, totalPlays = all shown)
  const [step, setStep] = useState(0);

  // Convert step to trick/play position
  const { currentTrickIdx, visiblePlaysInCurrentTrick, playedCardIds } = useMemo(() => {
    let remaining = step;
    let trickIdx = 0;
    const played = new Set<number>();

    for (let i = 0; i < sortedTricks.length; i++) {
      const plays = playsPerTrick.get(sortedTricks[i].trickId) ?? [];
      if (remaining <= plays.length) {
        // Add cards from plays shown so far in this trick
        for (let j = 0; j < remaining; j++) {
          for (const cid of plays[j].cardIds) played.add(cid);
        }
        return { currentTrickIdx: i, visiblePlaysInCurrentTrick: remaining, playedCardIds: played };
      }
      // All plays in this trick consumed
      for (const p of plays) {
        for (const cid of p.cardIds) played.add(cid);
      }
      remaining -= plays.length;
      trickIdx = i + 1;
    }
    return { currentTrickIdx: Math.min(trickIdx, sortedTricks.length - 1), visiblePlaysInCurrentTrick: 0, playedCardIds: played };
  }, [step, sortedTricks, playsPerTrick]);

  const goNext = useCallback(() => setStep(s => Math.min(s + 1, totalPlays)), [totalPlays]);
  const goPrev = useCallback(() => setStep(s => Math.max(s - 1, 0)), []);
  const goStart = useCallback(() => setStep(0), []);
  const goEnd = useCallback(() => setStep(totalPlays), [totalPlays]);

  // Cards remaining in each seat's hand at current step
  const handCards = useMemo(() => {
    const result: Record<number, Card[]> = { 0: [], 1: [], 2: [], 3: [] };
    for (const c of allCards) {
      if (!playedCardIds.has(c.cardId)) {
        result[c.ownerSeat].push(c);
      }
    }
    // Sort each hand
    for (const seat of [0, 1, 2, 3]) {
      result[seat].sort((a, b) => {
        if (a.suit !== b.suit) return b.suit - a.suit;
        return b.rank - a.rank;
      });
    }
    return result;
  }, [allCards, playedCardIds]);

  // Current trick view
  const currentTrick = sortedTricks[currentTrickIdx];
  const currentPlays = currentTrick ? (playsPerTrick.get(currentTrick.trickId) ?? []) : [];
  const visiblePlays = currentPlays.slice(0, visiblePlaysInCurrentTrick);

  const cardMap = useMemo(() => new Map(allCards.map(c => [c.cardId, c])), [allCards]);

  // Position helper
  function posClass(seat: number) {
    if (seat === relativeSeats.top) return 'trick-pos-top';
    if (seat === relativeSeats.left) return 'trick-pos-left';
    if (seat === relativeSeats.right) return 'trick-pos-right';
    if (seat === relativeSeats.bottom) return 'trick-pos-bottom';
    return '';
  }

  // Tricks won counter up to current step
  const tricksWon = useMemo(() => {
    const won: Record<number, number> = { 0: 0, 1: 0, 2: 0, 3: 0 };
    for (let i = 0; i < currentTrickIdx; i++) {
      const t = sortedTricks[i];
      if (t.winnerSeat != null) won[t.winnerSeat]++;
    }
    return won;
  }, [currentTrickIdx, sortedTricks]);

  // Group a hand by suit for display
  const groupBySuit = (cards: Card[]) => {
    const groups: [number, Card[]][] = [];
    for (const suit of [SPADES, HEARTS, DIAMONDS, CLUBS]) {
      const sc = cards.filter(c => c.suit === suit);
      if (sc.length > 0) groups.push([suit, sc]);
    }
    return groups;
  };

  return (
    <div className="replay-overlay">
      <div className="replay-container">
        <div className="replay-header">
          <h3>Hand Replay — Trick {currentTrickIdx + 1} of {sortedTricks.length}</h3>
          <button className="btn btn-secondary replay-close" onClick={onClose}>✕</button>
        </div>

        <div className="replay-table">
          {/* Four hands */}
          <div className="replay-hand replay-hand-top">
            <div className="replay-seat-label">{seatLabels[relativeSeats.top]} <span className="replay-tricks-won">({tricksWon[relativeSeats.top]})</span></div>
            <div className="replay-mini-hand">
              {groupBySuit(handCards[relativeSeats.top]).map(([suit, cards]) => (
                <span key={suit} className="replay-suit-group">
                  <span style={{ color: suitColorBright(suit) }}>{suitSymbol(suit)}</span>
                  {cards.map(c => <span key={c.cardId} className="replay-rank">{rankName(c.rank)}</span>)}
                </span>
              ))}
            </div>
          </div>

          <div className="replay-middle">
            <div className="replay-hand replay-hand-left">
              <div className="replay-seat-label">{seatLabels[relativeSeats.left]} <span className="replay-tricks-won">({tricksWon[relativeSeats.left]})</span></div>
              <div className="replay-mini-hand replay-mini-hand-vertical">
                {groupBySuit(handCards[relativeSeats.left]).map(([suit, cards]) => (
                  <span key={suit} className="replay-suit-group">
                    <span style={{ color: suitColorBright(suit) }}>{suitSymbol(suit)}</span>
                    {cards.map(c => <span key={c.cardId} className="replay-rank">{rankName(c.rank)}</span>)}
                  </span>
                ))}
              </div>
            </div>

            {/* Trick area */}
            <div className="replay-trick-area">
              {visiblePlays.length === 0 && (
                <div className="trick-empty">{step === 0 ? 'Press ▶ to begin' : 'Trick complete'}</div>
              )}
              {visiblePlays.map(play => {
                const pos = posClass(play.seat);
                const cards = play.cardIds.map(id => cardMap.get(id)).filter(Boolean) as Card[];
                const isWinner = currentTrick?.winnerSeat === play.seat && visiblePlaysInCurrentTrick === currentPlays.length;
                return (
                  <div key={play.playId} className={`trick-play ${pos} ${isWinner ? 'trick-winner' : ''}`}>
                    <span className="trick-player-name">{seatLabels[play.seat] ?? SEAT_NAMES[play.seat]}</span>
                    {play.isPass ? (
                      <div className="trick-cards">
                        <div className="trick-card-row">
                          <div className="card card-sm card-pass"><span className="card-pass-label">PASS</span></div>
                        </div>
                      </div>
                    ) : (
                      <div className="trick-cards">
                        {cards.length > 1 && (
                          <div className="combo-value" style={{ color: suitColorBright(cards[0].suit) }}>
                            {cards.reduce((s, c) => s + c.rank, 0)}{suitSymbol(cards[0].suit)}
                          </div>
                        )}
                        <div className="trick-card-row">
                          {cards.map(c => <CardView key={c.cardId} card={c} small />)}
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            <div className="replay-hand replay-hand-right">
              <div className="replay-seat-label">{seatLabels[relativeSeats.right]} <span className="replay-tricks-won">({tricksWon[relativeSeats.right]})</span></div>
              <div className="replay-mini-hand replay-mini-hand-vertical">
                {groupBySuit(handCards[relativeSeats.right]).map(([suit, cards]) => (
                  <span key={suit} className="replay-suit-group">
                    <span style={{ color: suitColorBright(suit) }}>{suitSymbol(suit)}</span>
                    {cards.map(c => <span key={c.cardId} className="replay-rank">{rankName(c.rank)}</span>)}
                  </span>
                ))}
              </div>
            </div>
          </div>

          <div className="replay-hand replay-hand-bottom">
            <div className="replay-seat-label">{seatLabels[relativeSeats.bottom]} <span className="replay-tricks-won">({tricksWon[relativeSeats.bottom]})</span></div>
            <div className="replay-mini-hand">
              {groupBySuit(handCards[relativeSeats.bottom]).map(([suit, cards]) => (
                <span key={suit} className="replay-suit-group">
                  <span style={{ color: suitColorBright(suit) }}>{suitSymbol(suit)}</span>
                  {cards.map(c => <span key={c.cardId} className="replay-rank">{rankName(c.rank)}</span>)}
                </span>
              ))}
            </div>
          </div>
        </div>

        {/* Controls */}
        <div className="replay-controls">
          <button className="btn btn-secondary" onClick={goStart} disabled={step === 0}>⏮</button>
          <button className="btn btn-secondary" onClick={goPrev} disabled={step === 0}>◀</button>
          <span className="replay-step-info">Play {step} / {totalPlays}</span>
          <button className="btn btn-secondary" onClick={goNext} disabled={step === totalPlays}>▶</button>
          <button className="btn btn-secondary" onClick={goEnd} disabled={step === totalPlays}>⏭</button>
        </div>

        {trump != null && (
          <div className="replay-trump">
            Trump: <span style={{ color: suitColorBright(trump) }}>{suitSymbol(trump)}</span>
          </div>
        )}
      </div>
    </div>
  );
}
