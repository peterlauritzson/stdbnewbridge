# Kortbridge — Game Design & Implementation Plan

> A bridge-inspired trick-taking card game with passing, card combinations, and spread-based bidding.  
> Multiplayer via SpacetimeDB. UI inspired by BBO / IntoBridge.

---

## 1. Game Rules Summary

### 1.1 Players & Teams
- 4 players seated North / East / South / West
- Two partnerships: N–S vs E–W (configurable seat assignment)

### 1.2 Deck & Card Values
- Standard 52-card deck
- Value: Ace = 14, King = 13, Queen = 12, Jack = 11, 10–2 = face value
- Suits: Spades (♠), Hearts (♥), Diamonds (♦), Clubs (♣)

### 1.3 Auction (Bidding Phase)
- Proceeds clockwise starting from dealer
- A **bid** specifies a **spread** and a **trump suit** (or No Trump)
  - E.g., `+3♠` means "our pair will take 3 more tricks than opponents, with spades as trump"
  - Higher spreads outbid lower ones; suit ranking as in bridge: ♣ < ♦ < ♥ < ♠ < NT
- Players may **pass**
- Auction ends after 3 consecutive passes (with at least one bid made)
- The winning bidder's pair is **declaring**; the winning bidder is **declarer**
- **Dummy**: undecided — flag `dummy_open: bool` in game config (default: plays normally / hidden)

### 1.4 Card Play Phase

#### Leading
- Declarer (or the player to declarer's left — configurable) leads the first trick
- The **led suit** is determined by the first non-pass play in a trick

#### On Your Turn (clockwise from leader)
You play **0 or more cards**:
- **0 cards** = pass (preserve your hand)
- **1 card** = standard play
- **2+ cards** = combination — all must be the **same suit**, play value = sum of ranks
  - (Configurable max: `max_combo_size`, default = 2; set to 52 for unlimited)

#### Following Suit
- If you play card(s), you **must follow the led suit** if you hold any card(s) of that suit
- If void in the led suit, you may play trump (ruff) or pass
- If playing a combination, all cards must be of the required suit

#### Trick Evaluation
1. Collect all non-pass plays
2. Any trump-suit play beats any non-trump play (regardless of value)
3. Among trump plays: highest sum wins
4. Among led-suit plays (if no trump played): highest sum wins
5. Ties: first player to play the tying value wins (positional advantage)
6. If **all four players pass**: configurable — default: trick is void, lead stays with current leader

#### Winning the Trick
- Winner collects the trick and leads the next trick

### 1.5 Running Out of Cards
- A player with **no cards** cannot play (automatic pass every turn)
- If a player wins the lead with their **last card**, lead transfers to **partner**
- If **neither partner** has cards remaining, the opposing pair automatically wins all remaining tricks (since they can't be contested)

### 1.6 Game End & Scoring
- **Contract made**: declarer's pair achieves the bid spread OR all cards are played
  - Spread = (tricks won by declaring pair) − (tricks won by defending pair)
- **Overtricks**: each card remaining in the declaring pair's hands counts as 1 overtrick
  - (They *chose* not to play those cards — rewarded for efficiency)
- **Undertricks**: if spread is not met, standard penalty (details TBD / configurable)
- **Game ends immediately** once the bid spread is mathematically achieved (configurable: `early_finish: bool`)

### 1.7 Configurable Rules (stored in `GameConfig` table)

| Parameter | Type | Default | Description |
|---|---|---|---|
| `max_combo_size` | u8 | 2 | Max cards playable in a combination |
| `dummy_open` | bool | false | Whether dummy's hand is visible |
| `early_finish` | bool | true | End game immediately when spread is met |
| `lead_after_bid` | enum | `LeftOfDeclarer` | Who leads first (LeftOfDeclarer / Declarer) |
| `all_pass_trick` | enum | `VoidTrick` | What happens if all pass (VoidTrick / LeaderWins) |
| `deck_size` | u8 | 52 | Number of cards in the deck |
| `cards_per_player` | u8 | 13 | Cards dealt to each player |

---

## 2. Architecture Overview

```
┌──────────────────────────────────────┐
│         React + TypeScript UI        │
│  (Card table, hand, bidding box,     │
│   trick area, scoreboard)            │
└──────────────┬───────────────────────┘
               │ SpacetimeDB TS SDK
               │ (WebSocket, auto-sync)
┌──────────────▼───────────────────────┐
│       SpacetimeDB Module (Rust)      │
│  Tables: Game, Player, Hand, Trick…  │
│  Reducers: create_game, bid,         │
│            play_cards, …             │
│  Logic: trick evaluation, game end   │
└──────────────────────────────────────┘
```

### Why SpacetimeDB
- Real-time sync via subscriptions — clients subscribe to game state and get automatic updates
- Server-authoritative logic in Rust reducers ensures no cheating
- Row-level access control so players only see their own cards (not opponents' hands)

---

## 3. Data Model (SpacetimeDB Tables)

### 3.1 `game_config`
Stores configurable rules per game.

| Column | Type | Notes |
|---|---|---|
| `game_id` | u64 (PK) | |
| `max_combo_size` | u8 | |
| `dummy_open` | bool | |
| `early_finish` | bool | |
| `lead_after_bid` | String | enum-like |
| `all_pass_trick` | String | enum-like |

### 3.2 `game`
Top-level game state.

| Column | Type | Notes |
|---|---|---|
| `game_id` | u64 (PK) | Auto-increment |
| `phase` | String | "lobby" / "auction" / "play" / "finished" |
| `dealer_seat` | u8 | 0–3 |
| `turn_seat` | u8 | Whose turn it is |
| `trump_suit` | Option\<u8\> | None = NT; 0–3 = ♣♦♥♠ |
| `contract_spread` | Option\<i8\> | The bid spread |
| `declarer_seat` | Option\<u8\> | |
| `ns_tricks` | u8 | Tricks taken by N–S |
| `ew_tricks` | u8 | Tricks taken by E–W |
| `current_trick_id` | u32 | FK to current trick |
| `result` | Option\<String\> | Final result when finished |

### 3.3 `player`
Links an identity to a seat.

| Column | Type | Notes |
|---|---|---|
| `identity` | Identity (PK) | SpacetimeDB caller identity |
| `game_id` | u64 | |
| `seat` | u8 | 0=N, 1=E, 2=S, 3=W |
| `name` | String | Display name |
| `online` | bool | Connection status |

### 3.4 `card`
Every card in the game — its location is tracked.

| Column | Type | Notes |
|---|---|---|
| `card_id` | u32 (PK) | Unique per game |
| `game_id` | u64 | |
| `suit` | u8 | 0–3 |
| `rank` | u8 | 2–14 |
| `location` | String | "hand:0" / "trick:5" / "won:1" |
| `play_order` | Option\<u8\> | Order within a trick play |

### 3.5 `bid`
One row per bid in the auction.

| Column | Type | Notes |
|---|---|---|
| `bid_id` | u32 (PK) | |
| `game_id` | u64 | |
| `seat` | u8 | |
| `spread` | Option\<i8\> | None = pass |
| `suit` | Option\<u8\> | None = NT or pass |
| `sequence` | u16 | Ordering within auction |

### 3.6 `trick`
One row per trick.

| Column | Type | Notes |
|---|---|---|
| `trick_id` | u32 (PK) | |
| `game_id` | u64 | |
| `trick_number` | u8 | 1-based |
| `leader_seat` | u8 | Who led |
| `led_suit` | Option\<u8\> | Set once first card is played |
| `winner_seat` | Option\<u8\> | Set when trick completes |

### 3.7 `trick_play`
One row per player's action in a trick (including pass).

| Column | Type | Notes |
|---|---|---|
| `play_id` | u32 (PK) | |
| `trick_id` | u32 | |
| `game_id` | u64 | |
| `seat` | u8 | |
| `is_pass` | bool | True if player passed |
| `card_ids` | Vec\<u32\> | Cards played (empty if pass) |
| `play_value` | u16 | Sum of card ranks (0 if pass) |
| `is_trump` | bool | Whether the play was in trump suit |
| `sequence` | u8 | 0–3 order within trick |

---

## 4. SpacetimeDB Reducers (Server API)

| Reducer | Parameters | Description |
|---|---|---|
| `create_game` | config params | Creates a game in "lobby" phase |
| `join_game` | game_id, name | Assigns caller to next open seat |
| `start_game` | game_id | Deals cards, moves to "auction" |
| `place_bid` | game_id, spread?, suit? | Place a bid or pass in auction |
| `play_cards` | game_id, card_ids: Vec | Play 0+ cards (empty = pass, 1+ = play) |
| `leave_game` | game_id | Disconnect / leave |

### Key Server Logic (in reducers)

#### `play_cards` — the single play reducer (0 cards = pass)
```
1. Validate it's the caller's turn & phase == "play"
2. If card_ids is empty → treat as pass (is_pass = true, skip to step 8)
3. Validate all cards are in the caller's hand
4. Validate all cards are the same suit
5. Validate count ≤ max_combo_size
6. If led_suit is set → validate cards follow suit (or player is void)
7. If led_suit is not set → this play sets the led_suit
8. Compute play_value = sum of ranks (0 for pass)
9. Record trick_play, move cards to trick location (if any)
10. Advance turn to next seat (skip players with empty hands)
11. If all 4 seats have played (or were skipped):
    a. Evaluate trick winner
    b. Award trick to winning pair
    c. Check game-end conditions (spread met, all cards played)
    d. If not ended: start next trick, set leader
```

#### `evaluate_trick` — pure function
```
1. Gather all non-pass plays for this trick
2. If no plays: handle per all_pass_trick config
3. Partition into trump plays and led-suit plays
4. If any trump plays: winner = highest value among trump plays
5. Else: winner = highest value among led-suit plays
6. Ties broken by earliest sequence number
7. Return winner_seat
```

---

## 5. Frontend Structure (React + TypeScript)

```
client/
├── src/
│   ├── generated/          # Auto-generated SpacetimeDB bindings
│   ├── components/
│   │   ├── GameTable.tsx       # Main game layout (BBO-like top-down view)
│   │   ├── PlayerHand.tsx      # Fan of cards in player's hand
│   │   ├── CardView.tsx        # Single card component (SVG or image-based)
│   │   ├── TrickArea.tsx       # Center area showing current trick plays
│   │   ├── BiddingBox.tsx      # Auction UI: bid buttons + pass
│   │   ├── BidHistory.tsx      # Auction log table
│   │   ├── Scoreboard.tsx      # NS vs EW tricks, contract info
│   │   ├── Lobby.tsx           # Game creation / joining
│   │   └── GameOver.tsx        # Results screen
│   ├── hooks/
│   │   ├── useSpacetimeDB.ts   # Connection & subscription management
│   │   └── useGameState.ts     # Derived state from subscribed tables
│   ├── lib/
│   │   ├── cardUtils.ts        # Card sorting, display names, suit symbols
│   │   └── gameLogic.ts        # Client-side helpers (valid plays, etc.)
│   ├── App.tsx
│   └── main.tsx
├── public/
│   └── cards/              # Card images/SVGs
├── package.json
└── tsconfig.json
```

### UI Layout (BBO-style)

```
┌─────────────────────────────────────────────┐
│                  North (top)                 │
│               [  card backs  ]              │
├──────────┬─────────────────────┬────────────┤
│          │                     │            │
│  West    │    TRICK AREA       │   East     │
│ [backs]  │   [played cards]    │  [backs]   │
│          │                     │            │
├──────────┴─────────────────────┴────────────┤
│              Your hand (South)              │
│  [  2♣  ] [  5♦  ] [  K♥  ] [  A♠  ] ...  │
├─────────────────────────────────────────────┤
│  Bidding Box / Info Bar / Scoreboard        │
└─────────────────────────────────────────────┘
```

- Cards playable are highlighted; invalid plays are greyed out
- Click to select cards for a combination, then confirm
- Pass button always available during play phase
- Current trick plays animate into center
- Previous tricks viewable in a side panel

---

## 6. Implementation Phases

### Phase 1 — Core Engine (Rust module) ✦ Priority
> Get the trick-playing mechanism completely correct

1. **Data model**: Define all SpacetimeDB tables and enums
2. **Deck & dealing**: Shuffle, assign 13 cards per player
3. **Trick play loop**: `play_cards` (0 cards = pass), turn advancement, skip empty hands
4. **Trick evaluation**: `evaluate_trick` — winner determination with trump, sums, ties
5. **Lead transfer**: handle last-card-wins → partner leads, both-out → opponents auto-win
6. **Game end detection**: spread achieved or all cards gone
7. **Unit tests**: exhaustive tests for trick evaluation edge cases

### Phase 2 — Auction System
1. Bidding reducer with validation (must outbid previous)
2. Auction end detection (3 passes)
3. Contract & declarer determination
4. Transition from auction → play phase

### Phase 3 — Lobby & Connection
1. `create_game` / `join_game` / `leave_game` reducers
2. Player identity and reconnection handling
3. Game Config table and defaults

### Phase 4 — Frontend Basics
1. SpacetimeDB connection setup
2. Lobby screen (create/join)
3. Card rendering (hand display, sorted by suit)
4. Game table layout

### Phase 5 — Frontend Play
1. Card selection UI (click to toggle, confirm play)
2. Pass button
3. Trick area animation
4. Turn indicator & valid-play highlighting
5. Bid box UI

### Phase 6 — Scoring & Polish
1. Score calculation (overtricks, undertricks)
2. Game-over screen with results
3. Previous trick viewer
4. Dummy hand display (if config enabled)
5. Sound effects, card animations

### Phase 7 — Stretch Goals
- Spectator mode
- Game replay
- Multiple rounds / rubber scoring
- Chat / emotes
- Timer per turn
- AI bot players

---

## 7. Key Design Decisions to Finalize

| # | Question | Current Default | Notes |
|---|---|---|---|
| 1 | Max combo size | 2 | Make configurable, start with 2 |
| 2 | Dummy hand | Hidden (plays normally) | Add `dummy_open` flag |
| 3 | All-pass trick | Void trick, leader retains lead | Could also be: leader wins |
| 4 | Who leads first | Left of declarer (bridge standard) | Configurable |
| 5 | Can you trump when passing is an option? | Yes, if void in led suit | Standard bridge void rule |
| 6 | Card visibility | Only own hand + played cards | Server filters via row-level access |
| 7 | Scoring formula | TBD | Start with simple spread ± overtricks |

---

## 8. File Structure (Full Project)

```
stdbnewbridge/
├── server/                    # SpacetimeDB Rust module
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # Module entry, table definitions
│       ├── game.rs            # Game lifecycle reducers
│       ├── auction.rs         # Bidding logic
│       ├── play.rs            # Card play reducers & trick evaluation
│       ├── deck.rs            # Card creation, shuffling, dealing
│       ├── types.rs           # Enums, shared types
│       └── config.rs          # GameConfig defaults & validation
├── client/                    # React + TypeScript frontend
│   ├── (see section 5)
│   └── ...
├── PLAN.md                    # This file
└── README.md
```

---

## 9. SpacetimeDB Specifics

- **Module publish**: `spacetime publish stdbnewbridge` (or local via `spacetime start` + `spacetime publish --local`)
- **Client SDK**: `@clockworklabs/spacetimedb-sdk` (npm)
- **Code generation**: `spacetime generate --lang typescript --out-dir client/src/generated`
- **Subscriptions**: Client subscribes to filtered queries — e.g., own hand, current trick, game state
- **Access control**: Reducers check `ctx.sender` against `player.identity` to enforce turns and hand ownership
- **Docs**: https://spacetimedb.com/docs

---

*This plan is a living document. Rules marked as configurable can be adjusted without restructuring the core engine.*
