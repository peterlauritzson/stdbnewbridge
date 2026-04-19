import type { TrickPlay, Card } from '../types';
import { SEAT_NAMES } from '../types';
import { CardView } from './CardView';
import { suitSymbol, suitColorBright } from '../lib/cardUtils';

interface TrickAreaProps {
  plays: TrickPlay[];
  allCards: Card[];
  relativeSeats: { top: number; left: number; right: number; bottom: number };
  seatLabels: Record<number, string>;
  leaderSeat?: number;
  winnerSeat?: number | null;
}

/** Maps a seat number to a CSS position class in the trick area */
function positionClass(seat: number, relativeSeats: TrickAreaProps['relativeSeats']): string {
  if (seat === relativeSeats.top) return 'trick-pos-top';
  if (seat === relativeSeats.left) return 'trick-pos-left';
  if (seat === relativeSeats.right) return 'trick-pos-right';
  if (seat === relativeSeats.bottom) return 'trick-pos-bottom';
  return '';
}

export function TrickArea({ plays, allCards, relativeSeats, seatLabels, winnerSeat }: TrickAreaProps) {
  // Build a map of card_id -> Card for quick lookup
  const cardMap = new Map(allCards.map(c => [c.cardId, c]));

  return (
    <div className="trick-area">
      {plays.length === 0 && (
        <div className="trick-empty">Waiting for play...</div>
      )}
      
      {plays.map(play => {
        const pos = positionClass(play.seat, relativeSeats);
        const isWinner = winnerSeat === play.seat;
        const playCards = play.cardIds.map(id => cardMap.get(id)).filter(Boolean) as Card[];

        return (
          <div key={play.playId} className={`trick-play ${pos} ${isWinner ? 'trick-winner' : ''}`}>
            <span className="trick-player-name">{seatLabels[play.seat] ?? SEAT_NAMES[play.seat]}</span>
            {play.isPass ? (
              <div className="trick-cards">
                <div className="trick-card-row">
                  <div className="card card-sm card-pass">
                    <span className="card-pass-label">PASS</span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="trick-cards">
                {playCards.length >= 1 && (
                  <div className="combo-value" style={{ color: suitColorBright(playCards[0].suit) }}>
                    {playCards.reduce((sum, c) => sum + c.rank, 0)}{suitSymbol(playCards[0].suit)}
                  </div>
                )}
                <div className="trick-card-row">
                  {playCards.map(card => (
                    <CardView key={card.cardId} card={card} small />
                  ))}
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
