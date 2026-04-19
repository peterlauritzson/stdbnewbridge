use game_logic::{logic, types};

use spacetimedb::{table, reducer, ReducerContext, Table};
use spacetimedb::rand::Rng;

// ============================================================
// Tables
// ============================================================

#[table(accessor = game_config, public)]
#[derive(Clone)]
pub struct GameConfig {
    #[primary_key]
    pub game_id: u64,
    pub max_combo_size: u8,
    pub dummy_open: bool,
    pub early_finish: bool,
    pub lead_after_bid: String, // "LeftOfDeclarer" | "Declarer"
    pub all_pass_trick: String, // "VoidTrick" | "LeaderWins"
}

#[table(accessor = game, public)]
#[derive(Clone)]
pub struct Game {
    #[primary_key]
    #[auto_inc]
    pub game_id: u64,
    pub phase: u8,
    pub dealer_seat: u8,
    pub turn_seat: u8,
    pub trump_suit: Option<u8>,       // None = NT; 0-3 = suit
    pub contract_spread: Option<i8>,
    pub declarer_seat: Option<u8>,
    pub ns_tricks: u8,
    pub ew_tricks: u8,
    pub current_trick_id: u32,
    pub result: Option<String>,
}

#[table(accessor = player, public)]
#[derive(Clone)]
pub struct Player {
    #[primary_key]
    pub identity: spacetimedb::Identity,
    pub game_id: u64,
    pub seat: u8,
    pub name: String,
    pub online: bool,
}

#[table(accessor = card, public)]
#[derive(Clone)]
pub struct Card {
    #[primary_key]
    #[auto_inc]
    pub card_id: u32,
    pub game_id: u64,
    #[index(btree)]
    pub owner_seat: u8,        // which seat this card was dealt to (immutable)
    pub suit: u8,
    pub rank: u8,
    pub location: String,      // "hand" / "trick:{trick_id}" / "won:{seat}"
}

#[table(accessor = bid, public)]
#[derive(Clone)]
pub struct Bid {
    #[primary_key]
    #[auto_inc]
    pub bid_id: u32,
    pub game_id: u64,
    pub seat: u8,
    pub spread: Option<i8>,   // None = pass
    pub suit: Option<u8>,     // None = NT or pass
    pub sequence: u16,
}

#[table(accessor = trick, public)]
#[derive(Clone)]
pub struct Trick {
    #[primary_key]
    #[auto_inc]
    pub trick_id: u32,
    pub game_id: u64,
    pub trick_number: u8,
    pub leader_seat: u8,
    pub led_suit: Option<u8>,
    pub winner_seat: Option<u8>,
}

#[table(accessor = trick_play, public)]
#[derive(Clone)]
pub struct TrickPlay {
    #[primary_key]
    #[auto_inc]
    pub play_id: u32,
    pub trick_id: u32,
    pub game_id: u64,
    pub seat: u8,
    pub is_pass: bool,
    pub card_ids: Vec<u32>,
    pub play_value: u16,
    pub play_suit: Option<u8>,
    pub sequence: u8,
}

#[table(accessor = ai_player, public)]
#[derive(Clone)]
pub struct AiPlayer {
    #[primary_key]
    #[auto_inc]
    pub ai_id: u32,
    pub game_id: u64,
    pub seat: u8,
    pub name: String,
}

// ============================================================
// Lifecycle reducers
// ============================================================

#[reducer(init)]
pub fn init(_ctx: &ReducerContext) {
    log::info!("Kortbridge module initialized");
}

#[reducer(client_connected)]
pub fn client_connected(_ctx: &ReducerContext) {
    log::info!("Client connected: {:?}", _ctx.sender());
}

#[reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    // Mark the player as offline
    if let Some(mut p) = ctx.db.player().identity().find(&ctx.sender()) {
        p.online = false;
        ctx.db.player().identity().update(p);
    }
}

// ============================================================
// AI helpers
// ============================================================

fn is_ai_seat(ctx: &ReducerContext, game_id: u64, seat: u8) -> bool {
    ctx.db.ai_player().iter().any(|a| a.game_id == game_id && a.seat == seat)
}

// ---- AI Auction Logic ----

/// High-card points: A=4, K=3, Q=2, J=1
fn hcp(rank: u8) -> u8 {
    match rank {
        14 => 4, // Ace
        13 => 3, // King
        12 => 2, // Queen
        11 => 1, // Jack
        _ => 0,
    }
}

/// Evaluate hand strength and pick a bid, or pass.
fn ai_choose_bid(
    ctx: &ReducerContext,
    game_id: u64,
    seat: u8,
) -> (Option<i8>, Option<u8>) {
    let hand: Vec<Card> = ctx.db.card().iter()
        .filter(|c| c.game_id == game_id && c.owner_seat == seat && c.location == "hand")
        .collect();

    // Count HCP
    let total_hcp: u8 = hand.iter().map(|c| hcp(c.rank)).sum();

    // Count cards per suit
    let mut suit_counts = [0u8; 4];
    let mut suit_hcp = [0u8; 4];
    for c in &hand {
        suit_counts[c.suit as usize] += 1;
        suit_hcp[c.suit as usize] += hcp(c.rank);
    }

    // Distribution points: each card beyond 4 in a suit = 1pt, void = 3, singleton = 2, doubleton = 1
    let mut dist_pts: u8 = 0;
    for &count in &suit_counts {
        if count == 0 { dist_pts += 3; }
        else if count == 1 { dist_pts += 2; }
        else if count == 2 { dist_pts += 1; }
        else if count > 4 { dist_pts += count - 4; }
    }

    let strength = total_hcp + dist_pts;

    // Need decent strength to bid (roughly: 12+ to open)
    if strength < 12 {
        return (None, None);
    }

    // Find longest suit (prefer higher-ranked suit for ties)
    let mut best_suit = 0u8;
    for s in 0..4u8 {
        if suit_counts[s as usize] > suit_counts[best_suit as usize]
            || (suit_counts[s as usize] == suit_counts[best_suit as usize] && suit_hcp[s as usize] > suit_hcp[best_suit as usize])
            || (suit_counts[s as usize] == suit_counts[best_suit as usize] && suit_hcp[s as usize] == suit_hcp[best_suit as usize] && s > best_suit)
        {
            best_suit = s;
        }
    }

    // If no suit has 4+ cards, bid NT
    let bid_suit: Option<u8> = if suit_counts[best_suit as usize] >= 4 {
        Some(best_suit)
    } else {
        None // NT
    };

    // Spread based on strength:
    // 12-15 → 1, 16-19 → 2, 20-23 → 3, 24-27 → 4, 28+ → 5
    let spread = ((strength as i8 - 12) / 4 + 1).min(7);

    // Check if this outranks the current high bid
    let highest_bid: Option<Bid> = ctx.db.bid().iter()
        .filter(|b| b.game_id == game_id && b.spread.is_some())
        .max_by_key(|b| b.sequence);

    if let Some(prev) = highest_bid {
        if !types::bid_outranks(spread, bid_suit, prev.spread.unwrap(), prev.suit) {
            // Can't outbid — pass
            return (None, None);
        }
    }

    (Some(spread), bid_suit)
}

// ---- AI Play Logic ----

/// Get the current best play in the trick (highest-value non-pass play
/// considering trump and led suit, like evaluate_trick but returns details).
fn current_trick_winner(
    ctx: &ReducerContext,
    trick: &Trick,
    game: &Game,
) -> Option<(u8, u16, Option<u8>)> {
    // Returns (winning_seat, winning_value, winning_suit)
    let plays: Vec<TrickPlay> = ctx.db.trick_play().iter()
        .filter(|tp| tp.trick_id == trick.trick_id && !tp.is_pass)
        .collect();

    if plays.is_empty() {
        return None;
    }

    let trump = game.trump_suit;
    let led = trick.led_suit;

    let mut best: Option<&TrickPlay> = None;
    for p in &plays {
        let dominated = match best {
            None => true,
            Some(b) => {
                let p_trump = trump.is_some() && p.play_suit == trump;
                let b_trump = trump.is_some() && b.play_suit == trump;
                let p_led = led.is_some() && p.play_suit == led;
                let b_led = led.is_some() && b.play_suit == led;

                if p_trump && !b_trump { true }
                else if !p_trump && b_trump { false }
                else if p_trump && b_trump { p.play_value > b.play_value || (p.play_value == b.play_value && p.sequence < b.sequence) }
                else if p_led && !b_led { true }
                else if !p_led && b_led { false }
                else { p.play_value > b.play_value || (p.play_value == b.play_value && p.sequence < b.sequence) }
            }
        };
        if dominated { best = Some(p); }
    }

    best.map(|b| (b.seat, b.play_value, b.play_suit))
}

/// Pick cards for the AI to play. Returns empty vec for a pass.
fn ai_select_cards(ctx: &ReducerContext, game_id: u64, seat: u8, trick: &Trick, game: &Game) -> Vec<u32> {
    let hand: Vec<Card> = ctx.db.card().iter()
        .filter(|c| c.game_id == game_id && c.owner_seat == seat && c.location == "hand")
        .collect();

    if hand.is_empty() {
        return vec![];
    }

    let is_leader = seat == trick.leader_seat;
    let trump = game.trump_suit;

    // Group hand by suit
    let mut by_suit: [Vec<&Card>; 4] = [vec![], vec![], vec![], vec![]];
    for c in &hand {
        by_suit[c.suit as usize].push(c);
    }
    // Sort each suit group by rank descending
    for group in &mut by_suit {
        group.sort_by(|a, b| b.rank.cmp(&a.rank));
    }

    // ----- LEADING -----
    if is_leader {
        return ai_lead(seat, &hand, &by_suit, trump, game);
    }

    // ----- FOLLOWING -----
    let winner = current_trick_winner(ctx, trick, game);

    // How many cards did the current winning play use?
    let winner_card_count: usize = ctx.db.trick_play().iter()
        .filter(|tp| tp.trick_id == trick.trick_id && !tp.is_pass)
        .map(|tp| tp.card_ids.len())
        .max()
        .unwrap_or(1);

    // Check if partner is currently winning
    let partner_winning = winner.as_ref().map_or(false, |(w, _, _)| types::same_team(seat, *w));

    // Determine which suit was led
    let led_suit = match trick.led_suit {
        Some(s) => s,
        None => {
            let mut all_sorted = hand.clone();
            all_sorted.sort_by_key(|c| c.rank);
            return vec![all_sorted[0].card_id];
        }
    };

    let have_led_suit = !by_suit[led_suit as usize].is_empty();

    if have_led_suit {
        // Must follow suit
        let suited = &by_suit[led_suit as usize]; // sorted rank desc

        if partner_winning {
            // Partner winning → PASS to save cards
            return vec![];
        }

        // Can we beat the current winner?
        if let Some((_, best_val, best_suit)) = winner {
            let best_is_trump = trump.is_some() && best_suit == trump;

            if best_is_trump && Some(led_suit) != trump {
                // Current winner played trump, we're following non-trump → can't beat
                return vec![];
            }

            // ECONOMY CHECK: decide max cards we're willing to spend.
            // Each trick = 1 point, so spending N cards to win 1 trick costs
            // us N-1 potential future tricks. Only spend more cards than the
            // opponent if we can do it cheaply.
            let max_cards_to_spend: usize = if winner_card_count >= 4 {
                // Opponent dumped 4+ cards for 1 trick — let them have it.
                // Only beat if we have a single card that tops it.
                1
            } else if winner_card_count == 3 {
                // 3-card combo: only beat with 1 card, maybe 2 if cheap
                // (cheapest 2 must sum above target)
                1
            } else if winner_card_count == 2 {
                // 2-card play: spend up to 2 cards
                2
            } else {
                // Single card: spend up to 2 cards
                2
            };

            // Try single card first (cheapest that beats)
            for c in suited.iter().rev() {
                if (c.rank as u16) > best_val {
                    return vec![c.card_id];
                }
            }

            // Try combo only if allowed by economy
            if max_cards_to_spend >= 2 {
                let combo = find_winning_combo(suited, best_val, max_cards_to_spend);
                if let Some(ids) = combo {
                    return ids;
                }
            }

            // Can't beat economically → PASS
            return vec![];
        }

        // No winner yet (we're the first non-pass follower) — play lowest
        return vec![suited.last().unwrap().card_id];
    }

    // ----- VOID IN LED SUIT -----

    if partner_winning {
        return vec![];
    }

    // ECONOMY CHECK for trumping: don't trump a huge combo
    // If opponent spent 3+ cards, let them have it (we save a trump for later)
    if winner_card_count >= 3 {
        return vec![];
    }

    // Can we trump?
    if let Some(ts) = trump {
        if !by_suit[ts as usize].is_empty() {
            let trumps = &by_suit[ts as usize];
            if let Some((_, best_val, best_suit)) = winner {
                if best_suit == Some(ts) {
                    // Winner has trump — need to over-trump
                    for t in trumps.iter().rev() {
                        if (t.rank as u16) > best_val {
                            return vec![t.card_id];
                        }
                    }
                    return vec![];
                }
            }
            // Winner is non-trump or no winner: play lowest trump
            return vec![trumps.last().unwrap().card_id];
        }
    }

    vec![]
}

/// Find the cheapest combo of cards from `suited` (sorted rank desc)
/// whose total value > target, using at most `max_size` cards.
fn find_winning_combo(suited: &[&Card], target_value: u16, max_size: usize) -> Option<Vec<u32>> {
    let limit = max_size.min(suited.len());
    for size in 2..=limit {
        let low_cards: Vec<&&Card> = suited.iter().rev().take(size).collect();
        let total: u16 = low_cards.iter().map(|c| c.rank as u16).sum();
        if total > target_value {
            return Some(low_cards.iter().map(|c| c.card_id).collect());
        }
    }
    None
}

/// AI leading logic: conserve cards, prefer single-card leads.
fn ai_lead(_seat: u8, hand: &[Card], by_suit: &[Vec<&Card>; 4], trump: Option<u8>, game: &Game) -> Vec<u32> {
    let total_cards = hand.len();

    // Endgame with very few cards: just lead our best single card
    if total_cards <= 3 {
        let mut best = &hand[0];
        for c in hand {
            if c.rank > best.rank { best = c; }
        }
        return vec![best.card_id];
    }

    // Find the best suit to lead
    let mut best_suit_idx: Option<usize> = None;
    let mut best_score: i16 = -1;

    for s in 0..4usize {
        if by_suit[s].is_empty() { continue; }

        let is_trump = trump == Some(s as u8);
        let length_bonus = by_suit[s].len() as i16;
        let top_hcp: i16 = by_suit[s].iter().take(3).map(|c| hcp(c.rank) as i16).sum();

        // Prefer suits with high-card strength + length;
        // penalise leading trump (save it for later)
        let score = top_hcp * 2 + length_bonus - if is_trump { 6 } else { 0 };

        if score > best_score {
            best_score = score;
            best_suit_idx = Some(s);
        }
    }

    // Also consider leading a low singleton to create a void
    // (void = can trump later)
    if trump.is_some() {
        for s in 0..4usize {
            if by_suit[s].len() == 1 && trump != Some(s as u8) {
                let c = by_suit[s][0];
                // Only worthwhile if we actually have trumps to exploit the void
                let trump_count = trump.map_or(0, |t| by_suit[t as usize].len());
                if trump_count >= 2 && c.rank <= 10 {
                    // Lead the singleton low card to void ourselves
                    return vec![c.card_id];
                }
            }
        }
    }

    let suit_idx = best_suit_idx.unwrap_or(0);
    let suit_cards = &by_suit[suit_idx];

    // CONSERVATIVE LEADING: almost always lead a single card.
    // Rationale: each trick = 1 point regardless of cards played.
    // Leading A alone: if it wins, we spent 1 card for 1 trick (great).
    // Leading A-K (27 value): hard to beat, but we spent 2 cards for 1 trick.
    // Leading A-K-Q (39): nearly unbeatable, but 3 cards for 1 trick (terrible).
    //
    // Only lead a 2-card combo (A-K) when:
    //   - We have 6+ cards in the suit (plenty to spare)
    //   - Top card is the Ace (rank 14)
    //   - We're the declaring side and need to push through tricks
    let is_declarer_side = game.declarer_seat
        .map_or(false, |d| types::same_team(_seat, d));

    let is_suit_trump = trump == Some(suit_idx as u8);

    if suit_cards.len() >= 6
        && suit_cards[0].rank == 14  // Ace
        && suit_cards[1].rank == 13  // King
        && is_declarer_side
        && !is_suit_trump
    {
        return vec![suit_cards[0].card_id, suit_cards[1].card_id];
    }

    // Default: lead single highest card from chosen suit
    vec![suit_cards[0].card_id]
}

fn run_ai_loop(ctx: &ReducerContext, game_id: u64) {
    // Safety limit to prevent infinite loops
    for _ in 0..200 {
        let game = match ctx.db.game().game_id().find(&game_id) {
            Some(g) => g,
            None => return,
        };

        if game.phase == types::PHASE_FINISHED || game.phase == types::PHASE_LOBBY {
            return;
        }

        if !is_ai_seat(ctx, game_id, game.turn_seat) {
            return;
        }

        if game.phase == types::PHASE_AUCTION {
            let (spread, suit) = ai_choose_bid(ctx, game_id, game.turn_seat);
            internal_place_bid(ctx, game_id, game.turn_seat, spread, suit);
        } else if game.phase == types::PHASE_PLAY {
            let trick = ctx.db.trick().trick_id().find(&game.current_trick_id)
                .expect("Trick not found for AI play");
            let card_ids = ai_select_cards(ctx, game_id, game.turn_seat, &trick, &game);
            internal_play_cards(ctx, game_id, game.turn_seat, card_ids);
        } else {
            return;
        }
    }
}

// ============================================================
// Lobby reducers
// ============================================================

#[reducer]
pub fn create_game(
    ctx: &ReducerContext,
    name: String,
    max_combo_size: u8,
    dummy_open: bool,
    early_finish: bool,
    lead_after_bid: String,
    all_pass_trick: String,
) {
    // Ensure player isn't already in a game
    assert!(ctx.db.player().identity().find(&ctx.sender()).is_none(), "Already in a game");

    let game = ctx.db.game().insert(Game {
        game_id: 0, // auto_inc
        phase: types::PHASE_LOBBY,
        dealer_seat: 0,
        turn_seat: 0,
        trump_suit: None,
        contract_spread: None,
        declarer_seat: None,
        ns_tricks: 0,
        ew_tricks: 0,
        current_trick_id: 0,
        result: None,
    });

    ctx.db.game_config().insert(GameConfig {
        game_id: game.game_id,
        max_combo_size: if max_combo_size == 0 { 13 } else { max_combo_size },
        dummy_open,
        early_finish,
        lead_after_bid: if lead_after_bid.is_empty() {
            types::LEAD_LEFT_OF_DECLARER.to_string()
        } else {
            lead_after_bid
        },
        all_pass_trick: if all_pass_trick.is_empty() {
            types::ALL_PASS_VOID.to_string()
        } else {
            all_pass_trick
        },
    });

    log::info!("Game {} created", game.game_id);

    // Auto-join the creator as seat 0
    ctx.db.player().insert(Player {
        identity: ctx.sender(),
        game_id: game.game_id,
        seat: 0,
        name: name.clone(),
        online: true,
    });

    log::info!("{} auto-joined game {} as seat 0", name, game.game_id);
}

#[reducer]
pub fn join_game(ctx: &ReducerContext, game_id: u64, name: String) {
    let game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_LOBBY, "Game already started");

    // Check player not already in a game
    assert!(ctx.db.player().identity().find(&ctx.sender()).is_none(), "Already in a game");

    // Find next open seat (check both human and AI seats)
    let mut taken_seats: Vec<u8> = ctx.db.player().iter()
        .filter(|p| p.game_id == game_id)
        .map(|p| p.seat)
        .collect();
    taken_seats.extend(
        ctx.db.ai_player().iter()
            .filter(|a| a.game_id == game_id)
            .map(|a| a.seat)
    );
    let seat = (0..4u8).find(|s| !taken_seats.contains(s))
        .expect("Game is full");

    ctx.db.player().insert(Player {
        identity: ctx.sender(),
        game_id,
        seat,
        name: name.clone(),
        online: true,
    });

    log::info!("{} joined game {} as seat {}", name, game_id, seat);
}

#[reducer]
pub fn leave_game(ctx: &ReducerContext, game_id: u64) {
    let player = ctx.db.player().identity().find(&ctx.sender())
        .expect("Not in a game");
    assert!(player.game_id == game_id, "Not in this game");

    let mut game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");

    ctx.db.player().identity().delete(&ctx.sender());

    // If game is in progress (auction or play), abort it
    if game.phase == types::PHASE_AUCTION || game.phase == types::PHASE_PLAY {
        game.phase = types::PHASE_FINISHED;
        game.result = Some(format!("{} left — game aborted", player.name));
        ctx.db.game().game_id().update(game);
        log::info!("Game {} aborted because {} left", game_id, player.name);
    }
}

#[reducer]
pub fn delete_game(ctx: &ReducerContext, game_id: u64) {
    let _game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");

    let human_players: Vec<Player> = ctx.db.player().iter()
        .filter(|p| p.game_id == game_id)
        .collect();

    // Allow deletion if:
    // - The caller is in the game, OR there are no human players
    let caller_in_game = human_players.iter().any(|p| p.identity == ctx.sender());
    let no_humans = human_players.is_empty();

    assert!(
        caller_in_game || no_humans,
        "Only a player in the game can delete it"
    );

    // Clean up all related data
    for p in human_players {
        ctx.db.player().identity().delete(&p.identity);
    }
    for a in ctx.db.ai_player().iter().filter(|a| a.game_id == game_id).collect::<Vec<_>>() {
        ctx.db.ai_player().ai_id().delete(&a.ai_id);
    }
    for c in ctx.db.card().iter().filter(|c| c.game_id == game_id).collect::<Vec<_>>() {
        ctx.db.card().card_id().delete(&c.card_id);
    }
    for b in ctx.db.bid().iter().filter(|b| b.game_id == game_id).collect::<Vec<_>>() {
        ctx.db.bid().bid_id().delete(&b.bid_id);
    }
    for tp in ctx.db.trick_play().iter().filter(|tp| tp.game_id == game_id).collect::<Vec<_>>() {
        ctx.db.trick_play().play_id().delete(&tp.play_id);
    }
    for t in ctx.db.trick().iter().filter(|t| t.game_id == game_id).collect::<Vec<_>>() {
        ctx.db.trick().trick_id().delete(&t.trick_id);
    }
    if let Some(_) = ctx.db.game_config().game_id().find(&game_id) {
        ctx.db.game_config().game_id().delete(&game_id);
    }
    ctx.db.game().game_id().delete(&game_id);

    log::info!("Game {} deleted by {:?}", game_id, ctx.sender());
}

#[reducer]
pub fn seat_ai(ctx: &ReducerContext, game_id: u64) {
    let game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_LOBBY, "Game already started");

    let mut taken: Vec<u8> = ctx.db.player().iter()
        .filter(|p| p.game_id == game_id)
        .map(|p| p.seat)
        .collect();
    taken.extend(
        ctx.db.ai_player().iter()
            .filter(|a| a.game_id == game_id)
            .map(|a| a.seat)
    );

    let ai_names = ["AI-North", "AI-East", "AI-South", "AI-West"];
    for seat in 0..4u8 {
        if !taken.contains(&seat) {
            ctx.db.ai_player().insert(AiPlayer {
                ai_id: 0,
                game_id,
                seat,
                name: ai_names[seat as usize].to_string(),
            });
            log::info!("AI seated at {} in game {}", ai_names[seat as usize], game_id);
        }
    }
}

// ============================================================
// Game start & dealing
// ============================================================

#[reducer]
pub fn start_game(ctx: &ReducerContext, game_id: u64) {
    let mut game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_LOBBY, "Game already started");

    // Verify 4 seats filled (human + AI)
    let human_count = ctx.db.player().iter()
        .filter(|p| p.game_id == game_id)
        .count();
    let ai_count = ctx.db.ai_player().iter()
        .filter(|a| a.game_id == game_id)
        .count();
    assert!(human_count + ai_count == 4, "Need exactly 4 players (human + AI)");

    // Create and shuffle deck
    let mut deck: Vec<(u8, u8)> = Vec::with_capacity(52);
    for suit in 0..4u8 {
        for rank in 2..=14u8 {
            deck.push((suit, rank));
        }
    }

    // Fisher-Yates shuffle using SpacetimeDB's deterministic RNG
    for i in (1..deck.len()).rev() {
        let j = ctx.rng().gen_range(0..=i);
        deck.swap(i, j);
    }

    // Deal 13 cards to each player
    for (i, (suit, rank)) in deck.iter().enumerate() {
        let seat = (i / 13) as u8;
        ctx.db.card().insert(Card {
            card_id: 0, // auto_inc
            game_id,
            owner_seat: seat,
            suit: *suit,
            rank: *rank,
            location: "hand".to_string(),
        });
    }

    // Move to auction phase
    game.phase = types::PHASE_AUCTION;
    game.turn_seat = types::next_seat(game.dealer_seat); // left of dealer bids first
    ctx.db.game().game_id().update(game);

    log::info!("Game {} started, auction begins", game_id);

    // Auto-play AI turns if an AI seat is first to act
    run_ai_loop(ctx, game_id);
}

// ============================================================
// Auction / play internal helpers
// ============================================================

fn internal_place_bid(ctx: &ReducerContext, game_id: u64, seat: u8, spread: Option<i8>, suit: Option<u8>) {
    let mut game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_AUCTION, "Not in auction phase");
    assert!(seat == game.turn_seat, "Not this seat's turn");

    // Validate suit range if provided
    if let Some(s) = suit {
        assert!(s <= 3, "Invalid suit");
    }

    let is_pass = spread.is_none();

    if !is_pass {
        let bid_spread = spread.unwrap();
        assert!(bid_spread > 0, "Spread must be positive");

        // Must outbid the current highest bid
        let highest_bid: Option<Bid> = ctx.db.bid().iter()
            .filter(|b| b.game_id == game_id && b.spread.is_some())
            .max_by_key(|b| b.sequence);

        if let Some(prev) = highest_bid {
            assert!(
                types::bid_outranks(bid_spread, suit, prev.spread.unwrap(), prev.suit),
                "Bid must outrank the previous bid"
            );
        }
    }

    // Count existing bids for sequence
    let bid_count = ctx.db.bid().iter()
        .filter(|b| b.game_id == game_id)
        .count() as u16;

    ctx.db.bid().insert(Bid {
        bid_id: 0, // auto_inc
        game_id,
        seat,
        spread,
        suit,
        sequence: bid_count,
    });

    // Check if auction is over: 3 consecutive passes after at least one real bid
    let has_real_bid = ctx.db.bid().iter()
        .any(|b| b.game_id == game_id && b.spread.is_some());

    let recent_three_passes = if bid_count + 1 >= 3 {
        let mut bids: Vec<Bid> = ctx.db.bid().iter()
            .filter(|b| b.game_id == game_id)
            .collect();
        bids.sort_by_key(|b| b.sequence);
        let len = bids.len();
        bids[len - 1].spread.is_none()
            && bids[len - 2].spread.is_none()
            && bids[len - 3].spread.is_none()
    } else {
        false
    };

    if has_real_bid && recent_three_passes {
        // Auction over — find the winning bid
        let winning_bid = ctx.db.bid().iter()
            .filter(|b| b.game_id == game_id && b.spread.is_some())
            .max_by_key(|b| b.sequence)
            .expect("Must have a real bid");

        game.contract_spread = winning_bid.spread;
        game.trump_suit = winning_bid.suit;
        game.declarer_seat = Some(winning_bid.seat);

        let config = ctx.db.game_config().game_id().find(&game_id)
            .expect("Config not found");

        // Determine who leads
        let leader = if config.lead_after_bid == types::LEAD_LEFT_OF_DECLARER {
            types::next_seat(winning_bid.seat)
        } else {
            winning_bid.seat
        };

        // Create the first trick
        let trick = ctx.db.trick().insert(Trick {
            trick_id: 0, // auto_inc
            game_id,
            trick_number: 1,
            leader_seat: leader,
            led_suit: None,
            winner_seat: None,
        });

        game.phase = types::PHASE_PLAY;
        game.turn_seat = leader;
        game.current_trick_id = trick.trick_id;
        ctx.db.game().game_id().update(game);

        log::info!("Auction complete for game {}. Contract: {:?} {:?}. Declarer: {}",
            game_id, winning_bid.spread, winning_bid.suit, winning_bid.seat);
    } else {
        // Check for 4 consecutive passes with no real bid — all pass
        if !has_real_bid && bid_count + 1 >= 4 {
            game.phase = types::PHASE_FINISHED;
            game.result = Some("All passed — no contract".to_string());
            ctx.db.game().game_id().update(game);
            log::info!("Game {} ended: all passed", game_id);
        } else {
            // Advance turn
            game.turn_seat = types::next_seat(game.turn_seat);
            ctx.db.game().game_id().update(game);
        }
    }
}

#[reducer]
pub fn place_bid(ctx: &ReducerContext, game_id: u64, spread: Option<i8>, suit: Option<u8>) {
    let player = ctx.db.player().identity().find(&ctx.sender())
        .expect("Not in this game");
    assert!(player.game_id == game_id, "Not in this game");
    let game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(player.seat == game.turn_seat, "Not your turn");
    internal_place_bid(ctx, game_id, player.seat, spread, suit);
    run_ai_loop(ctx, game_id);
}

// ============================================================
// Card play reducer
// ============================================================

fn internal_play_cards(ctx: &ReducerContext, game_id: u64, seat: u8, card_ids: Vec<u32>) {
    let mut game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_PLAY, "Not in play phase");
    assert!(seat == game.turn_seat, "Not this seat's turn");

    let config = ctx.db.game_config().game_id().find(&game_id)
        .expect("Config not found");

    let mut trick = ctx.db.trick().trick_id().find(&game.current_trick_id)
        .expect("Current trick not found");

    let is_pass = card_ids.is_empty();

    // Leader of the trick must play at least one card
    if is_pass {
        assert!(seat != trick.leader_seat, "Leader must play at least one card");
    }

    let play_suit: Option<u8>;
    let play_value: u16;

    if is_pass {
        play_suit = None;
        play_value = 0;
    } else {
        // Validate card count
        assert!(
            card_ids.len() as u8 <= config.max_combo_size,
            "Too many cards (max {})", config.max_combo_size
        );

        // Fetch all played cards and validate ownership
        let mut cards_played: Vec<Card> = Vec::new();
        for &cid in &card_ids {
            let c = ctx.db.card().card_id().find(&cid)
                .expect("Card not found");
            assert!(c.game_id == game_id, "Card not in this game");
            assert!(c.owner_seat == seat, "Not your card");
            assert!(c.location == "hand", "Card not in hand");
            cards_played.push(c);
        }

        // All cards must be the same suit
        let suit = cards_played[0].suit;
        assert!(cards_played.iter().all(|c| c.suit == suit), "All cards must be the same suit");

        // Follow suit rule
        if let Some(led) = trick.led_suit {
            if suit != led {
                // Check player is truly void in led suit
                let has_led = ctx.db.card().iter()
                    .any(|c| c.game_id == game_id && c.owner_seat == seat && c.location == "hand" && c.suit == led);
                assert!(!has_led, "Must follow the led suit");
            }
        }

        play_suit = Some(suit);
        play_value = cards_played.iter().map(|c| c.rank as u16).sum();

        // Set led_suit if this is the first card play of the trick
        if trick.led_suit.is_none() {
            trick.led_suit = Some(suit);
            ctx.db.trick().trick_id().update(trick.clone());
        }

        // Move cards from hand to trick
        for &cid in &card_ids {
            let mut c = ctx.db.card().card_id().find(&cid).unwrap();
            c.location = format!("trick:{}", trick.trick_id);
            ctx.db.card().card_id().update(c);
        }
    }

    // Count plays already in this trick
    let play_count = ctx.db.trick_play().iter()
        .filter(|tp| tp.trick_id == trick.trick_id)
        .count() as u8;

    // Record the trick play
    ctx.db.trick_play().insert(TrickPlay {
        play_id: 0, // auto_inc
        trick_id: trick.trick_id,
        game_id,
        seat,
        is_pass,
        card_ids: card_ids.clone(),
        play_value,
        play_suit,
        sequence: play_count,
    });

    // Determine cards remaining per seat
    let hand_cards: Vec<(u8, u8, u8)> = ctx.db.card().iter()
        .filter(|c| c.game_id == game_id && c.location == "hand")
        .map(|c| (c.owner_seat, c.suit, c.rank))
        .collect();
    let cards_remaining = logic::count_cards_per_seat(&hand_cards);

    // Count how many seats have now played in this trick (including auto-skipped)
    let total_plays = play_count + 1;

    // Check if all 4 seats have acted (played or were auto-skipped)
    // We need to track: plays made + seats with 0 cards that haven't played
    let seats_played: Vec<u8> = ctx.db.trick_play().iter()
        .filter(|tp| tp.trick_id == trick.trick_id)
        .map(|tp| tp.seat)
        .collect();

    let all_seats_done = (0..4u8).all(|s| {
        seats_played.contains(&s) || cards_remaining[s as usize] == 0
    });

    if all_seats_done || total_plays >= 4 {
        // Auto-play passes for empty-hand seats that haven't played yet
        for s in 0..4u8 {
            if !seats_played.contains(&s) && cards_remaining[s as usize] == 0 {
                let auto_seq = ctx.db.trick_play().iter()
                    .filter(|tp| tp.trick_id == trick.trick_id)
                    .count() as u8;
                ctx.db.trick_play().insert(TrickPlay {
                    play_id: 0,
                    trick_id: trick.trick_id,
                    game_id,
                    seat: s,
                    is_pass: true,
                    card_ids: vec![],
                    play_value: 0,
                    play_suit: None,
                    sequence: auto_seq,
                });
            }
        }

        // Evaluate the trick
        let plays: Vec<logic::TrickPlayInfo> = ctx.db.trick_play().iter()
            .filter(|tp| tp.trick_id == trick.trick_id)
            .map(|tp| logic::TrickPlayInfo {
                seat: tp.seat,
                is_pass: tp.is_pass,
                play_value: tp.play_value,
                suit: tp.play_suit,
                sequence: tp.sequence,
            })
            .collect();

        let trick_result = logic::evaluate_trick(&plays, game.trump_suit, trick.led_suit);

        match trick_result.winner_seat {
            None => {
                // All passed — handle per config
                if config.all_pass_trick == types::ALL_PASS_LEADER_WINS {
                    // Leader "wins" (gets the trick, though it's empty)
                    if types::is_ns(trick.leader_seat) {
                        game.ns_tricks += 1;
                    } else {
                        game.ew_tricks += 1;
                    }
                    trick.winner_seat = Some(trick.leader_seat);
                }
                // VoidTrick: no trick awarded, leader stays
                ctx.db.trick().trick_id().update(trick.clone());
            }
            Some(winner) => {
                trick.winner_seat = Some(winner);
                ctx.db.trick().trick_id().update(trick.clone());

                // Award trick
                if types::is_ns(winner) {
                    game.ns_tricks += 1;
                } else {
                    game.ew_tricks += 1;
                }

                // Move trick cards to "won:{seat}" 
                let trick_cards: Vec<Card> = ctx.db.card().iter()
                    .filter(|c| c.game_id == game_id && c.location == format!("trick:{}", trick.trick_id))
                    .collect();
                for mut c in trick_cards {
                    c.location = format!("won:{}", winner);
                    ctx.db.card().card_id().update(c);
                }
            }
        }

        // Recalculate cards remaining after this trick
        let hand_cards_post: Vec<(u8, u8, u8)> = ctx.db.card().iter()
            .filter(|c| c.game_id == game_id && c.location == "hand")
            .map(|c| (c.owner_seat, c.suit, c.rank))
            .collect();
        let cards_post = logic::count_cards_per_seat(&hand_cards_post);
        let total_cards: u8 = cards_post.iter().sum();

        // Check game end conditions
        let mut game_over = false;

        if total_cards == 0 {
            // All cards played out
            game_over = true;
        }

        // Check if one pair has no cards (opponents sweep)
        let ns_cards = cards_post[types::NORTH as usize] + cards_post[types::SOUTH as usize];
        let ew_cards = cards_post[types::EAST as usize] + cards_post[types::WEST as usize];
        if ns_cards == 0 && ew_cards > 0 {
            // EW auto-wins remaining tricks (each card = part of a trick)
            // Simplified: remaining cards / average per trick, but since
            // we can't know exact tricks, give EW credit for remaining
            game.ew_tricks += ew_cards; // each remaining card counts as a trick
            game_over = true;
        } else if ew_cards == 0 && ns_cards > 0 {
            game.ns_tricks += ns_cards;
            game_over = true;
        }

        // Check spread achievement (early finish)
        if !game_over && config.early_finish {
            if let Some(contract) = game.contract_spread {
                let total_tricks = game.ns_tricks + game.ew_tricks + (total_cards / 4).max(1);
                if let Some(made) = logic::check_spread(
                    game.ns_tricks, game.ew_tricks, contract,
                    game.declarer_seat.unwrap_or(0), total_tricks
                ) {
                    if made {
                        game_over = true;
                    }
                    // If contract failed, we still play on (defenders may want overtricks)
                    // Actually per rules: game ends if spread is met
                }
            }
        }

        if game_over {
            finish_game(ctx, &mut game);
        } else {
            // Start next trick
            let leader_seat = if let Some(winner) = trick_result.winner_seat {
                match logic::resolve_leader(winner, &cards_post) {
                    Some(l) => l,
                    None => {
                        // Both partners of winning pair out — opponents get rest
                        // This should have been caught above, but handle gracefully
                        finish_game(ctx, &mut game);
                        return;
                    }
                }
            } else {
                // All passed, void trick — leader stays
                trick.leader_seat
            };

            let new_trick = ctx.db.trick().insert(Trick {
                trick_id: 0,
                game_id,
                trick_number: trick.trick_number + 1,
                leader_seat,
                led_suit: None,
                winner_seat: None,
            });

            game.current_trick_id = new_trick.trick_id;
            game.turn_seat = leader_seat;
            ctx.db.game().game_id().update(game);
        }
    } else {
        // More players need to act — advance to next active seat
        let next = logic::next_active_seat(seat, &cards_remaining)
            .expect("No active player found");
        game.turn_seat = next;
        ctx.db.game().game_id().update(game);
    }
}

#[reducer]
pub fn play_cards(ctx: &ReducerContext, game_id: u64, card_ids: Vec<u32>) {
    let player = ctx.db.player().identity().find(&ctx.sender())
        .expect("Not in this game");
    assert!(player.game_id == game_id, "Not in this game");
    let game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(player.seat == game.turn_seat, "Not your turn");
    internal_play_cards(ctx, game_id, player.seat, card_ids);
    run_ai_loop(ctx, game_id);
}

// ============================================================
// Game completion
// ============================================================

fn finish_game(ctx: &ReducerContext, game: &mut Game) {
    game.phase = types::PHASE_FINISHED;

    let declarer_seat = game.declarer_seat.unwrap_or(0);
    let declarer_is_ns = types::is_ns(declarer_seat);
    let (decl_tricks, def_tricks) = if declarer_is_ns {
        (game.ns_tricks, game.ew_tricks)
    } else {
        (game.ew_tricks, game.ns_tricks)
    };

    let spread = decl_tricks as i8 - def_tricks as i8;
    let contract = game.contract_spread.unwrap_or(0);

    // Count remaining cards in declaring pair's hands as overtricks
    let hand_cards: Vec<(u8, u8, u8)> = ctx.db.card().iter()
        .filter(|c| c.game_id == game.game_id && c.location == "hand")
        .map(|c| (c.owner_seat, c.suit, c.rank))
        .collect();
    let cards_remaining = logic::count_cards_per_seat(&hand_cards);

    let decl_remaining = if declarer_is_ns {
        cards_remaining[types::NORTH as usize] + cards_remaining[types::SOUTH as usize]
    } else {
        cards_remaining[types::EAST as usize] + cards_remaining[types::WEST as usize]
    };

    let result = if spread >= contract {
        let overtricks = (spread - contract) as u8 + decl_remaining;
        format!("Contract made! Spread: {}. Overtricks: {}", spread, overtricks)
    } else {
        let undertricks = contract - spread;
        format!("Contract failed. Spread: {}. Undertricks: {}", spread, undertricks)
    };

    game.result = Some(result.clone());
    ctx.db.game().game_id().update(game.clone());

    log::info!("Game {} finished: {}", game.game_id, result);
}

// ============================================================
// Next hand — reset for a new deal, rotate dealer
// ============================================================

#[reducer]
pub fn next_hand(ctx: &ReducerContext, game_id: u64) {
    let player = ctx.db.player().identity().find(&ctx.sender())
        .expect("Not in this game");
    assert!(player.game_id == game_id, "Not in this game");

    let mut game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_FINISHED, "Game not finished yet");

    // Delete old cards, bids, trick_plays, tricks for this game
    let old_cards: Vec<Card> = ctx.db.card().iter()
        .filter(|c| c.game_id == game_id).collect();
    for c in old_cards { ctx.db.card().card_id().delete(&c.card_id); }

    let old_bids: Vec<Bid> = ctx.db.bid().iter()
        .filter(|b| b.game_id == game_id).collect();
    for b in old_bids { ctx.db.bid().bid_id().delete(&b.bid_id); }

    let old_trick_plays: Vec<TrickPlay> = ctx.db.trick_play().iter()
        .filter(|tp| tp.game_id == game_id).collect();
    for tp in old_trick_plays { ctx.db.trick_play().play_id().delete(&tp.play_id); }

    let old_tricks: Vec<Trick> = ctx.db.trick().iter()
        .filter(|t| t.game_id == game_id).collect();
    for t in old_tricks { ctx.db.trick().trick_id().delete(&t.trick_id); }

    // Rotate dealer one seat clockwise
    let new_dealer = types::next_seat(game.dealer_seat);

    // Reset game state
    game.phase = types::PHASE_LOBBY;
    game.dealer_seat = new_dealer;
    game.turn_seat = 0;
    game.trump_suit = None;
    game.contract_spread = None;
    game.declarer_seat = None;
    game.ns_tricks = 0;
    game.ew_tricks = 0;
    game.current_trick_id = 0;
    game.result = None;
    ctx.db.game().game_id().update(game.clone());

    log::info!("Game {} reset for next hand. New dealer: seat {}", game_id, new_dealer);

    // Auto-start: re-deal and begin since all players are still seated
    let human_count = ctx.db.player().iter()
        .filter(|p| p.game_id == game_id).count();
    let ai_count = ctx.db.ai_player().iter()
        .filter(|a| a.game_id == game_id).count();
    if human_count + ai_count == 4 {
        // Re-deal cards
        let mut deck: Vec<(u8, u8)> = Vec::with_capacity(52);
        for suit in 0..4u8 {
            for rank in 2..=14u8 {
                deck.push((suit, rank));
            }
        }
        for i in (1..deck.len()).rev() {
            let j = ctx.rng().gen_range(0..=i);
            deck.swap(i, j);
        }
        for (i, (suit, rank)) in deck.iter().enumerate() {
            let seat = (i / 13) as u8;
            ctx.db.card().insert(Card {
                card_id: 0,
                game_id,
                owner_seat: seat,
                suit: *suit,
                rank: *rank,
                location: "hand".to_string(),
            });
        }

        game.phase = types::PHASE_AUCTION;
        game.turn_seat = types::next_seat(new_dealer);
        ctx.db.game().game_id().update(game);

        log::info!("Game {} auto-started next hand, auction begins", game_id);
        run_ai_loop(ctx, game_id);
    }
}
