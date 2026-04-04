import type { Game } from '../types';
import { SEAT_NAMES } from '../types';

interface GameOverProps {
  game: Game;
  seatLabels: Record<number, string>;
  onBackToLobby: () => void;
}

export function GameOver({ game, seatLabels, onBackToLobby }: GameOverProps) {
  const nsWon = game.nsTricks > game.ewTricks;

  return (
    <div className="game-over">
      <h2>Game Over</h2>
      <div className="result-card">
        <div className="result-winner">
          {nsWon ? 'North-South' : 'East-West'} wins!
        </div>
        <div className="result-details">
          <div className="result-row">
            <span>NS Tricks:</span>
            <strong>{game.nsTricks}</strong>
          </div>
          <div className="result-row">
            <span>EW Tricks:</span>
            <strong>{game.ewTricks}</strong>
          </div>
          {game.result && (
            <div className="result-row">
              <span>Result:</span>
              <strong>{game.result}</strong>
            </div>
          )}
          {game.declarerSeat != null && (
            <div className="result-row">
              <span>Declarer:</span>
              <strong>{seatLabels[game.declarerSeat] ?? SEAT_NAMES[game.declarerSeat]}</strong>
            </div>
          )}
        </div>
      </div>
      <button className="btn btn-primary" onClick={onBackToLobby}>
        Back to Lobby
      </button>
    </div>
  );
}
