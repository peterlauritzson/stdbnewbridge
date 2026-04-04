import type { Game } from '../types';
import { trumpName } from '../lib/cardUtils';
import { SEAT_NAMES, PHASE_LOBBY, PHASE_AUCTION, PHASE_PLAY, PHASE_FINISHED } from '../types';

const PHASE_NAMES: Record<number, string> = {
  [PHASE_LOBBY]: 'Lobby',
  [PHASE_AUCTION]: 'Auction',
  [PHASE_PLAY]: 'Play',
  [PHASE_FINISHED]: 'Finished',
};

interface ScoreboardProps {
  game: Game;
  seatLabels: Record<number, string>;
}

export function Scoreboard({ game, seatLabels }: ScoreboardProps) {
  return (
    <div className="scoreboard">
      <div className="scoreboard-row">
        <span className="label">Contract:</span>
        <span className="value">
          {game.contractSpread != null
            ? `${game.contractSpread} ${trumpName(game.trumpSuit ?? null)}`
            : '—'}
        </span>
      </div>
      {game.declarerSeat != null && (
        <div className="scoreboard-row">
          <span className="label">Declarer:</span>
          <span className="value">{seatLabels[game.declarerSeat] ?? SEAT_NAMES[game.declarerSeat]}</span>
        </div>
      )}
      <div className="scoreboard-row">
        <span className="label">NS Tricks:</span>
        <span className="value">{game.nsTricks}</span>
      </div>
      <div className="scoreboard-row">
        <span className="label">EW Tricks:</span>
        <span className="value">{game.ewTricks}</span>
      </div>
      <div className="scoreboard-row">
        <span className="label">Phase:</span>
        <span className="value">{PHASE_NAMES[game.phase] ?? game.phase}</span>
      </div>
      {game.turnSeat !== undefined && (
        <div className="scoreboard-row">
          <span className="label">Turn:</span>
          <span className="value">{seatLabels[game.turnSeat] ?? SEAT_NAMES[game.turnSeat]}</span>
        </div>
      )}
    </div>
  );
}
