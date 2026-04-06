import { useState, useEffect, useRef } from 'react';
import { useSpacetime } from '../hooks/useSpacetimeDB';
import { useGameState } from '../hooks/useGameState';
import { PlayerHand } from './PlayerHand';
import { TrickArea } from './TrickArea';
import { BiddingBox, BidHistory } from './BiddingBox';
import { Scoreboard } from './Scoreboard';
import { PHASE_LOBBY, PHASE_AUCTION, PHASE_PLAY, PHASE_FINISHED, SEAT_NAMES } from '../types';
import type { TrickPlay } from '../types';
import { trumpName, groupBySuit, suitSymbol, suitColor, rankName } from '../lib/cardUtils';

interface GameTableProps {
  gameId: bigint;
  onLeave: () => void;
}

export function GameTable({ gameId, onLeave }: GameTableProps) {
  const { state, actions } = useSpacetime();
  const gs = useGameState(gameId);

  // Track previous completed trick to show briefly before moving on
  const [frozenTrick, setFrozenTrick] = useState<{
    plays: TrickPlay[];
    leaderSeat?: number;
    winnerSeat?: number;
  } | null>(null);
  const prevTrickIdRef = useRef<number | null>(null);

  // Panel collapse state
  const [scoreboardOpen, setScoreboardOpen] = useState(true);
  const [biddingOpen, setBiddingOpen] = useState(true);
  const [historyOpen, setHistoryOpen] = useState(true);
  // Last trick viewer
  const [showLastTrick, setShowLastTrick] = useState(false);
  const lastCompletedTrickRef = useRef<{
    plays: TrickPlay[];
    leaderSeat?: number;
    winnerSeat?: number;
  } | null>(null);

  useEffect(() => {
    const currentTrickId = gs.currentTrick?.trickId ?? null;
    const prevId = prevTrickIdRef.current;

    // When trick changes and we had a previous trick, show it briefly
    if (prevId !== null && currentTrickId !== null && currentTrickId !== prevId) {
      // Find the old trick's plays from state
      const oldTrick = state.tricks.find(t => t.trickId === prevId && t.gameId === gameId);
      const oldPlays = state.trickPlays
        .filter(tp => tp.trickId === prevId && tp.gameId === gameId)
        .sort((a, b) => a.sequence - b.sequence);

      if (oldPlays.length > 0) {
        const trickData = {
          plays: oldPlays,
          leaderSeat: oldTrick?.leaderSeat,
          winnerSeat: oldTrick?.winnerSeat,
        };
        setFrozenTrick(trickData);
        lastCompletedTrickRef.current = trickData;
        const timer = setTimeout(() => setFrozenTrick(null), 2000);
        prevTrickIdRef.current = currentTrickId;
        return () => clearTimeout(timer);
      }
    }

    prevTrickIdRef.current = currentTrickId;
  }, [gs.currentTrick?.trickId, gameId, state.tricks, state.trickPlays]);

  if (!gs.currentGame) {
    return (
      <div className="game-table-loading">
        <p>Loading game...</p>
        <button className="btn btn-secondary" onClick={onLeave}>Back to Lobby</button>
      </div>
    );
  }

  const game = gs.currentGame;

  // Finished — don't return early, we'll show overlay on game table

  // Lobby / Waiting for players
  if (game.phase === PHASE_LOBBY) {
    const aiPlayers = state.aiPlayers.filter(a => a.gameId === gameId);
    const totalSeated = gs.gamePlayers.length + aiPlayers.length;
    return (
      <div className="game-table-lobby">
        <h2>Game #{String(game.gameId)} — Waiting for Players</h2>
        <div className="seat-grid">
          {[0, 1, 2, 3].map(seat => {
            const player = gs.gamePlayers.find(p => p.seat === seat);
            const ai = aiPlayers.find(a => a.seat === seat);
            return (
              <div key={seat} className={`seat-slot seat-${SEAT_NAMES[seat].toLowerCase()}`}>
                <div className="seat-name">{SEAT_NAMES[seat]}</div>
                {player ? (
                  <div className={`seat-player ${player.online ? 'online' : 'offline'}`}>
                    {player.name}
                  </div>
                ) : ai ? (
                  <div className="seat-player ai">🤖 {ai.name}</div>
                ) : (
                  <div className="seat-empty">Empty</div>
                )}
              </div>
            );
          })}
        </div>
        <div className="game-table-actions">
          {totalSeated < 4 && (
            <button className="btn btn-ai" onClick={() => actions.seatAi(gameId)}>
              🤖 Seat AI in Empty Chairs
            </button>
          )}
          {totalSeated === 4 && (
            <button className="btn btn-primary" onClick={() => actions.startGame(gameId)}>
              Start Game
            </button>
          )}
          <button className="btn btn-secondary" onClick={() => { actions.leaveGame(gameId); onLeave(); }}>
            Leave Game
          </button>
        </div>
      </div>
    );
  }

  // Get opponent hands (card backs)
  const opponentCards = (seat: number) =>
    gs.allCards.filter(c => c.ownerSeat === seat && c.location === 'hand');

  const { top, left, right, bottom } = gs.relativeSeats;

  // Current high bid for the bidding box
  const highBid = gs.gameBids.length > 0
    ? [...gs.gameBids].reverse().find(b => b.spread !== null) ?? null
    : null;

  // Determine which trick to display
  const lastTrick = lastCompletedTrickRef.current;
  const viewingLastTrick = showLastTrick && lastTrick;
  const displayTrickPlays = viewingLastTrick ? lastTrick.plays : (frozenTrick ? frozenTrick.plays : gs.currentTrickPlays);
  const displayLeaderSeat = viewingLastTrick ? lastTrick.leaderSeat : (frozenTrick ? frozenTrick.leaderSeat : gs.currentTrick?.leaderSeat);
  const displayWinnerSeat = viewingLastTrick ? lastTrick.winnerSeat : (frozenTrick ? frozenTrick.winnerSeat : gs.currentTrick?.winnerSeat);

  return (
    <div className="game-table">
      {/* Top opponent */}
      <div className="table-top">
        <div className="player-label">{gs.seatLabels[top]}</div>
        <PlayerHand cards={opponentCards(top)} isMyHand={false} />
      </div>

      {/* Middle row: left, trick area, right */}
      <div className="table-middle">
        <div className="table-left">
          <div className="player-label">{gs.seatLabels[left]}</div>
          <PlayerHand cards={opponentCards(left)} isMyHand={false} />
        </div>

        <TrickArea
          plays={displayTrickPlays}
          allCards={gs.allCards}
          relativeSeats={gs.relativeSeats}
          seatLabels={gs.seatLabels}
          leaderSeat={displayLeaderSeat}
          winnerSeat={displayWinnerSeat}
        />

        <div className="table-right">
          <div className="player-label">{gs.seatLabels[right]}</div>
          <PlayerHand cards={opponentCards(right)} isMyHand={false} />
        </div>
      </div>

      {/* Bottom: our hand */}
      <div className="table-bottom">
        <div className="player-label player-label-self">
          {gs.seatLabels[bottom]} (You)
          {gs.isMyTurn && <span className="turn-indicator"> — Your Turn!</span>}
        </div>
        <PlayerHand
          cards={gs.myHand}
          isMyHand={true}
          playable={!frozenTrick && !viewingLastTrick && gs.isMyTurn && game.phase === PHASE_PLAY}
          ledSuit={gs.currentTrick?.ledSuit}
          isLeader={gs.mySeat != null && gs.currentTrick?.leaderSeat === gs.mySeat}
          onPlay={(cardIds) => actions.playCards(gameId, cardIds)}
          onPass={() => actions.playCards(gameId, [])}
        />
      </div>

      {/* ---- Floating corner panels ---- */}

      {/* Scoreboard — top-right */}
      <div className={`panel panel-scoreboard${scoreboardOpen ? '' : ' panel-collapsed'}`}>
        <div className="panel-header" onClick={() => setScoreboardOpen(o => !o)}>
          <span>Scoreboard</span>
          <span className="panel-toggle">{scoreboardOpen ? '▲' : '▼'}</span>
        </div>
        <div className="panel-body">
          <Scoreboard game={game} seatLabels={gs.seatLabels} />
          <button className="btn btn-secondary btn-leave-panel" onClick={onLeave}>
            Back to Lobby
          </button>
        </div>
      </div>

      {/* Bidding Box — bottom-right (auction only) */}
      {game.phase === PHASE_AUCTION && (
        <div className={`panel panel-bidding${biddingOpen ? '' : ' panel-collapsed'}`}>
          <div className="panel-header" onClick={() => setBiddingOpen(o => !o)}>
            <span>Bidding</span>
            <span className="panel-toggle">{biddingOpen ? '▲' : '▼'}</span>
          </div>
          <div className="panel-body">
            <BiddingBox
              isMyTurn={gs.isMyTurn}
              currentHighBid={highBid}
              onBid={(spread, suit) => actions.placeBid(gameId, spread, suit)}
              onPass={() => actions.placeBid(gameId, null, null)}
            />
          </div>
        </div>
      )}

      {/* Bid History — top-left */}
      {(game.phase === PHASE_AUCTION || game.phase === PHASE_PLAY) && (
        <div className={`panel panel-history${historyOpen ? '' : ' panel-collapsed'}`}>
          <div className="panel-header" onClick={() => setHistoryOpen(o => !o)}>
            <span>Bid History</span>
            <span className="panel-toggle">{historyOpen ? '▲' : '▼'}</span>
          </div>
          <div className="panel-body">
            <BidHistory bids={gs.gameBids} seatLabels={gs.seatLabels} />
          </div>
        </div>
      )}

      {/* Last Trick viewer — bottom-left */}
      {game.phase === PHASE_PLAY && lastTrick && (
        <div className="panel panel-lasttrick">
          <div className="panel-header" onClick={() => setShowLastTrick(v => !v)}>
            <span>{viewingLastTrick ? 'Viewing Last Trick' : 'Last Trick'}</span>
            <span className="panel-toggle">{viewingLastTrick ? '✕' : '👁'}</span>
          </div>
        </div>
      )}

      {/* Hand result overlay */}
      {game.phase === PHASE_FINISHED && (
        <div className="result-overlay">
          <div className="result-overlay-card">
            <h2>Hand Complete</h2>
            <div className="result-details">
              <div className="result-row">
                <span>Contract:</span>
                <strong>
                  {game.contractSpread != null
                    ? `${game.contractSpread} ${trumpName(game.trumpSuit ?? null)}`
                    : 'All Passed'}
                </strong>
              </div>
              {game.declarerSeat != null && (
                <div className="result-row">
                  <span>Declarer:</span>
                  <strong>{gs.seatLabels[game.declarerSeat] ?? SEAT_NAMES[game.declarerSeat]}</strong>
                </div>
              )}
              <div className="result-row">
                <span>NS Tricks:</span>
                <strong>{game.nsTricks}</strong>
              </div>
              <div className="result-row">
                <span>EW Tricks:</span>
                <strong>{game.ewTricks}</strong>
              </div>
              {game.result && (
                <div className="result-row result-message">
                  {game.result}
                </div>
              )}
            </div>

            {/* Original dealt hands */}
            <div className="result-hands">
              {[0, 1, 2, 3].map(seat => {
                const seatCards = gs.allCards.filter(c => c.ownerSeat === seat);
                const grouped = groupBySuit(seatCards);
                return (
                  <div key={seat} className="result-hand">
                    <div className="result-hand-label">{gs.seatLabels[seat] ?? SEAT_NAMES[seat]}</div>
                    <div className="result-hand-suits">
                      {[...grouped.entries()].map(([suit, cards]) => (
                        <div key={suit} className="result-suit-line">
                          <span className="result-suit-symbol" style={{ color: suitColor(suit) }}>
                            {suitSymbol(suit)}
                          </span>
                          <span className="result-suit-cards">
                            {cards.map(c => rankName(c.rank)).join(' ')}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="result-actions">
              <button className="btn btn-primary" onClick={() => actions.nextHand(gameId)}>
                Next Hand
              </button>
              <button className="btn btn-secondary" onClick={onLeave}>
                Back to Lobby
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
