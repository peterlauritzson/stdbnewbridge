// Re-export generated types from SpacetimeDB module bindings
export type { Bid, Card, Game, GameConfig, Player, Trick, TrickPlay } from './module_bindings/types';

// Client-side constants (not in generated bindings)
export const CLUBS = 0;
export const DIAMONDS = 1;
export const HEARTS = 2;
export const SPADES = 3;

export const NORTH = 0;
export const EAST = 1;
export const SOUTH = 2;
export const WEST = 3;

export const SEAT_NAMES = ['North', 'East', 'South', 'West'] as const;

export const PHASE_LOBBY = 0;
export const PHASE_AUCTION = 1;
export const PHASE_PLAY = 2;
export const PHASE_FINISHED = 3;
