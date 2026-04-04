/**
 * SpacetimeDB connection management using generated module bindings.
 * Uses the SpacetimeDB React integration (spacetimedb/react).
 */

import { type ReactNode, useMemo } from 'react';
import { SpacetimeDBProvider, useSpacetimeDB, useTable } from 'spacetimedb/react';
import { DbConnection, tables } from '../module_bindings';
import type { Bid, Card, Game, GameConfig, Player, Trick, TrickPlay } from '../types';

export interface AiPlayer {
  aiId: number;
  gameId: bigint;
  seat: number;
  name: string;
}

export interface SpacetimeState {
  connected: boolean;
  identity: string | null;
  games: readonly Game[];
  gameConfigs: readonly GameConfig[];
  players: readonly Player[];
  aiPlayers: readonly AiPlayer[];
  cards: readonly Card[];
  bids: readonly Bid[];
  tricks: readonly Trick[];
  trickPlays: readonly TrickPlay[];
  error: string | null;
}

export interface SpacetimeActions {
  createGame: (name: string) => void;
  joinGame: (gameId: bigint, name: string) => void;
  leaveGame: (gameId: bigint) => void;
  deleteGame: (gameId: bigint) => void;
  startGame: (gameId: bigint) => void;
  seatAi: (gameId: bigint) => void;
  placeBid: (gameId: bigint, spread: number | null, suit: number | null) => void;
  playCards: (gameId: bigint, cardIds: number[]) => void;
  nextHand: (gameId: bigint) => void;
}

interface SpacetimeContextValue {
  state: SpacetimeState;
  actions: SpacetimeActions;
}

export function SpacetimeProvider({
  host,
  moduleName,
  children,
}: {
  host: string;
  moduleName: string;
  children: ReactNode;
}) {
  const builder = useMemo(() => {
    const savedToken = localStorage.getItem('stdb_token');
    let b = DbConnection.builder()
      .withUri(host.replace(/^http/, 'ws'))
      .withDatabaseName(moduleName)
      .onConnect((_conn, identity, token) => {
        console.log('[SpacetimeDB] Connected:', identity.toHexString());
        localStorage.setItem('stdb_token', token);
      })
      .onDisconnect(() => {
        console.log('[SpacetimeDB] Disconnected');
      })
      .onConnectError((_ctx, error) => {
        console.error('[SpacetimeDB] Connection error:', error);
      });
    if (savedToken) {
      b = b.withToken(savedToken);
    }
    return b;
  }, [host, moduleName]);

  return (
    <SpacetimeDBProvider connectionBuilder={builder}>
      {children}
    </SpacetimeDBProvider>
  );
}

export function useSpacetime(): SpacetimeContextValue {
  const { isActive, identity, getConnection } = useSpacetimeDB();
  const conn = getConnection() as DbConnection | null;

  // Subscribe to all tables reactively via useTable
  const [games] = useTable(tables.game);
  const [gameConfigs] = useTable(tables.game_config);
  const [players] = useTable(tables.player);
  const [aiPlayersRaw] = useTable(tables.ai_player);
  const [cards] = useTable(tables.card);
  const [bids] = useTable(tables.bid);
  const [tricks] = useTable(tables.trick);
  const [trickPlays] = useTable(tables.trick_play);

  const aiPlayers: AiPlayer[] = useMemo(() =>
    aiPlayersRaw.map((a: any) => ({ aiId: a.aiId, gameId: a.gameId, seat: a.seat, name: a.name })),
    [aiPlayersRaw]
  );

  const state: SpacetimeState = useMemo(() => ({
    connected: isActive,
    identity: identity?.toHexString() ?? null,
    games,
    gameConfigs,
    players,
    aiPlayers,
    cards,
    bids,
    tricks,
    trickPlays,
    error: null,
  }), [isActive, identity, games, gameConfigs, players, aiPlayers, cards, bids, tricks, trickPlays]);

  const actions: SpacetimeActions = useMemo(() => ({
    createGame: (name: string) => {
      conn?.reducers.createGame({ name, maxComboSize: 13, dummyOpen: false, earlyFinish: true, leadAfterBid: '', allPassTrick: '' });
    },
    joinGame: (gameId: bigint, name: string) => {
      conn?.reducers.joinGame({ gameId, name });
    },
    leaveGame: (gameId: bigint) => {
      conn?.reducers.leaveGame({ gameId });
    },
    deleteGame: (gameId: bigint) => {
      conn?.reducers.deleteGame({ gameId });
    },
    startGame: (gameId: bigint) => {
      conn?.reducers.startGame({ gameId });
    },
    seatAi: (gameId: bigint) => {
      conn?.reducers.seatAi({ gameId });
    },
    placeBid: (gameId: bigint, spread: number | null, suit: number | null) => {
      conn?.reducers.placeBid({ gameId, spread: spread ?? undefined, suit: suit ?? undefined });
    },
    playCards: (gameId: bigint, cardIds: number[]) => {
      conn?.reducers.playCards({ gameId, cardIds });
    },
    nextHand: (gameId: bigint) => {
      conn?.reducers.nextHand({ gameId });
    },
  }), [conn]);

  return { state, actions };
}
