import type { Card } from '../types';
import { rankName, suitSymbol, suitColor } from '../lib/cardUtils';

interface CardViewProps {
  card?: Card;
  faceDown?: boolean;
  selected?: boolean;
  playable?: boolean;
  small?: boolean;
  onClick?: () => void;
}

export function CardView({ card, faceDown, selected, playable, small, onClick }: CardViewProps) {
  if (faceDown || !card) {
    return (
      <div className={`card card-back ${small ? 'card-sm' : ''}`}>
        <div className="card-back-pattern">🂠</div>
      </div>
    );
  }

  const color = suitColor(card.suit);
  const rank = rankName(card.rank);
  const suit = suitSymbol(card.suit);

  return (
    <div
      className={[
        'card',
        small ? 'card-sm' : '',
        selected ? 'card-selected' : '',
        playable ? 'card-playable' : '',
        playable === false ? 'card-disabled' : '',
        onClick ? 'card-clickable' : '',
      ].filter(Boolean).join(' ')}
      style={{ color }}
      onClick={onClick}
    >
      <div className="card-corner card-corner-top">
        <span className="card-rank">{rank}</span>
        <span className="card-suit">{suit}</span>
      </div>
      <div className="card-center">
        <span className="card-suit-large">{suit}</span>
      </div>
      <div className="card-corner card-corner-bottom">
        <span className="card-rank">{rank}</span>
        <span className="card-suit">{suit}</span>
      </div>
    </div>
  );
}
