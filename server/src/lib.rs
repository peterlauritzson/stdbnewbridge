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

fn ai_select_cards(ctx: &ReducerContext, game_id: u64, seat: u8, trick: &Trick) -> Vec<u32> {
    let mut hand: Vec<Card> = ctx.db.card().iter()
        .filter(|c| c.game_id == game_id && c.owner_seat == seat && c.location == "hand")
        .collect();

    if hand.is_empty() {
        return vec![];
    }

    // Must follow suit if possible
    if let Some(led) = trick.led_suit {
        let mut suited: Vec<&Card> = hand.iter().filter(|c| c.suit == led).collect();
        if !suited.is_empty() {
            suited.sort_by_key(|c| c.rank);
            return vec![suited[0].card_id];
        }
    }

    // Play lowest card overall
    hand.sort_by_key(|c| c.rank);
    vec![hand[0].card_id]
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
            // AI always passes
            internal_place_bid(ctx, game_id, game.turn_seat, None, None);
        } else if game.phase == types::PHASE_PLAY {
            let trick = ctx.db.trick().trick_id().find(&game.current_trick_id)
                .expect("Trick not found for AI play");
            let card_ids = ai_select_cards(ctx, game_id, game.turn_seat, &trick);
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

    let game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");
    assert!(game.phase == types::PHASE_LOBBY, "Cannot leave after game started");

    ctx.db.player().identity().delete(&ctx.sender());
}

#[reducer]
pub fn delete_game(ctx: &ReducerContext, game_id: u64) {
    let game = ctx.db.game().game_id().find(&game_id)
        .expect("Game not found");

    let human_players: Vec<Player> = ctx.db.player().iter()
        .filter(|p| p.game_id == game_id)
        .collect();

    // Allow deletion if:
    // - Game is in Lobby or Finished phase
    // - The caller is in the game, OR there are no human players
    let caller_in_game = human_players.iter().any(|p| p.identity == ctx.sender());
    let no_humans = human_players.is_empty();

    assert!(
        game.phase == types::PHASE_LOBBY || game.phase == types::PHASE_FINISHED,
        "Cannot delete a game in progress"
    );
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
