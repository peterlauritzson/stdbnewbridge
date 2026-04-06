import { useState } from 'react';
import { useSpacetime } from '../hooks/useSpacetimeDB';
import { SEAT_NAMES, PHASE_LOBBY, PHASE_FINISHED } from '../types';

interface LobbyProps {
  onJoinGame: (gameId: bigint) => void;
}

export function Lobby({ onJoinGame }: LobbyProps) {
  const { state, actions } = useSpacetime();
  const [playerName, setPlayerName] = useState('');

  const handleCreateGame = () => {
    if (!playerName.trim()) return;
    actions.createGame(playerName.trim());
  };

  const handleJoinGame = (gameId: bigint) => {
    if (!playerName.trim()) return;
    actions.joinGame(gameId, playerName.trim());
    onJoinGame(gameId);
  };

  // Check if we're already in a game
  const myGame = state.identity
    ? state.players.find(p => p.identity.toHexString() === state.identity)
    : null;

  return (
    <div className="lobby">
      <h1>🃏 Kortbridge</h1>
      <p className="subtitle">A Swedish Bridge-like Card Game</p>

      <div className="ai-info">
        <details>
          <summary>🤖 About AI Players</summary>
          <ul>
            <li><strong>Auction:</strong> Bids based on hand strength (HCP + distribution). Needs 12+ points to open. Passes with weak hands.</li>
            <li><strong>Leading:</strong> Leads from its strongest suit, playing top sequences as combos (e.g. AKQ together).</li>
            <li><strong>Following:</strong> Passes when partner is winning. Only plays to beat the current winner — passes otherwise to save cards.</li>
            <li><strong>Trumping:</strong> Trumps in when void in led suit and can win. Won't waste trump if it can't over-trump.</li>
          </ul>
        </details>
      </div>

      {state.error && (
        <div className="error-banner">
          <strong>Note:</strong> {state.error}
        </div>
      )}

      <div className="lobby-form">
        <label>
          Your Name:
          <input
            type="text"
            value={playerName}
            onChange={e => setPlayerName(e.target.value)}
            placeholder="Enter your name"
            maxLength={20}
          />
        </label>

        <button className="btn btn-primary" onClick={handleCreateGame} disabled={!playerName.trim()}>
          Create New Game
        </button>
      </div>

      {myGame && (
        <div className="rejoin-banner">
          <span>You are seated in Game #{String(myGame.gameId)}</span>
          <button
            className="btn btn-primary"
            onClick={() => onJoinGame(myGame.gameId)}
          >
            Enter Game
          </button>
        </div>
      )}

      <div className="game-list">
        <h2>Open Games</h2>
        {state.games.length === 0 ? (
          <p className="empty-state">No games available. Create one!</p>
        ) : (
          <ul>
            {state.games.map(game => {
              const players = state.players.filter(p => p.gameId === game.gameId);
              const aiPlayers = state.aiPlayers.filter(a => a.gameId === game.gameId);
              const totalSeated = players.length + aiPlayers.length;
              const amInThisGame = myGame && myGame.gameId === game.gameId;
              return (
                <li key={String(game.gameId)} className="game-list-item">
                  <div className="game-info">
                    <span className="game-id">Game #{String(game.gameId)}</span>
                    <span className="game-phase">{game.phase}</span>
                    <span className="player-count">{totalSeated}/4 players</span>
                  </div>
                  <div className="game-players">
                    {players.map(p => (
                      <span key={p.identity.toHexString()} className={`player-badge ${p.online ? 'online' : 'offline'}`}>
                        {p.name} ({SEAT_NAMES[p.seat]})
                      </span>
                    ))}
                    {aiPlayers.map(a => (
                      <span key={`ai-${a.seat}`} className="player-badge ai">
                        🤖 {a.name} ({SEAT_NAMES[a.seat]})
                      </span>
                    ))}
                  </div>
                  <div className="game-actions">
                    {amInThisGame ? (
                      <button
                        className="btn btn-primary"
                        onClick={() => onJoinGame(game.gameId)}
                      >
                        Enter Game
                      </button>
                    ) : (
                      <button
                        className="btn btn-secondary"
                        onClick={() => handleJoinGame(game.gameId)}
                        disabled={!playerName.trim() || totalSeated >= 4 || !!myGame}
                      >
                        Join
                      </button>
                    )}
                    {amInThisGame && totalSeated < 4 && game.phase === PHASE_LOBBY && (
                      <button
                        className="btn btn-ai"
                        onClick={() => actions.seatAi(game.gameId)}
                      >
                        🤖 Seat AI
                      </button>
                    )}
                    {amInThisGame && totalSeated === 4 && game.phase === PHASE_LOBBY && (
                      <button
                        className="btn btn-primary"
                        onClick={() => {
                          actions.startGame(game.gameId);
                          onJoinGame(game.gameId);
                        }}
                      >
                        Start Game
                      </button>
                    )}
                    {(game.phase === PHASE_LOBBY || game.phase === PHASE_FINISHED) &&
                     (amInThisGame || players.length === 0) && (
                      <button
                        className="btn btn-danger"
                        onClick={() => actions.deleteGame(game.gameId)}
                      >
                        Delete Game
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
