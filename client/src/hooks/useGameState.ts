import { useMemo } from 'react';
import { useSpacetime } from './useSpacetimeDB';
import type { Card, Game, Player, Bid, Trick, TrickPlay } from '../types';
import { PHASE_AUCTION, PHASE_PLAY, SEAT_NAMES } from '../types';

export interface DerivedGameState {
  /** The game we're currently in (if any) */
  currentGame: Game | null;
  /** Our seat (or null if spectating) */
  mySeat: number | null;
  /** Our player record */
  myPlayer: Player | null;
  /** All players in the current game */
  gamePlayers: Player[];
  /** Cards in our hand */
  myHand: Card[];
  /** All cards in the game (for trick display, etc.) */
  allCards: Card[];
  /** Bids in the current game, sorted by sequence */
  gameBids: Bid[];
  /** Current trick */
  currentTrick: Trick | null;
  /** Plays in the current trick */
  currentTrickPlays: TrickPlay[];
  /** Is it our turn? */
  isMyTurn: boolean;
  /** Display name for each seat position relative to our seat */
  seatLabels: Record<number, string>;
  /** Seat positions relative to us (who's at top/left/right/bottom) */
  relativeSeats: { top: number; left: number; right: number; bottom: number };
}

/**
 * Derives useful game state from raw SpacetimeDB table data.
 * The player's own seat is always shown at the bottom.
 */
export function useGameState(gameId: bigint | null): DerivedGameState {
  const { state } = useSpacetime();

  return useMemo(() => {
    const currentGame = gameId !== null
      ? state.games.find(g => g.gameId === gameId) ?? null
      : null;

    const gamePlayers = state.players.filter(p =>
      currentGame && p.gameId === currentGame.gameId
    );

    const myPlayer = state.identity
      ? gamePlayers.find(p => p.identity.toHexString() === state.identity) ?? null
      : null;

    const mySeat = myPlayer?.seat ?? null;

    const allCards = state.cards.filter(c =>
      currentGame && c.gameId === currentGame.gameId
    );

    const myHand = mySeat !== null
      ? allCards.filter(c => c.ownerSeat === mySeat && c.location === 'hand')
      : [];

    const gameBids = state.bids
      .filter(b => currentGame && b.gameId === currentGame.gameId)
      .sort((a, b) => a.sequence - b.sequence);

    const currentTrick = currentGame
      ? state.tricks.find(t =>
          t.gameId === currentGame.gameId && t.trickId === currentGame.currentTrickId
        ) ?? null
      : null;

    const currentTrickPlays = currentTrick
      ? state.trickPlays
          .filter(tp => tp.trickId === currentTrick.trickId && tp.gameId === currentGame!.gameId)
          .sort((a, b) => a.sequence - b.sequence)
      : [];

    const isMyTurn = currentGame !== null
      && mySeat !== null
      && currentGame.turnSeat === mySeat
      && (currentGame.phase === PHASE_AUCTION || currentGame.phase === PHASE_PLAY);

    // Rotate seats so our seat is at the bottom
    // Standard bridge: LHO on left, partner on top, RHO on right
    const effectiveSeat = mySeat ?? 2; // default to south if spectating
    const relativeSeats = {
      bottom: effectiveSeat,
      left: (effectiveSeat + 1) % 4,   // LHO (next in clockwise order)
      top: (effectiveSeat + 2) % 4,    // Partner (opposite)
      right: (effectiveSeat + 3) % 4,  // RHO (previous in clockwise order)
    };

    const seatLabels: Record<number, string> = {};
    for (let s = 0; s < 4; s++) {
      const player = gamePlayers.find(p => p.seat === s);
      if (player) {
        seatLabels[s] = player.name;
      } else {
        const ai = state.aiPlayers.find(a => currentGame && a.gameId === currentGame.gameId && a.seat === s);
        seatLabels[s] = ai ? `🤖 ${ai.name}` : SEAT_NAMES[s];
      }
    }

    return {
      currentGame,
      mySeat,
      myPlayer,
      gamePlayers,
      myHand,
      allCards,
      gameBids,
      currentTrick,
      currentTrickPlays,
      isMyTurn,
      seatLabels,
      relativeSeats,
    };
  }, [state, gameId]);
}
