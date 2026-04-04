/// Trick evaluation logic — pure functions, no database dependency.
use crate::types;

/// Represents a single player's play in a trick.
#[derive(Debug, Clone)]
pub struct TrickPlayInfo {
    pub seat: u8,
    pub is_pass: bool,
    pub play_value: u16,   // sum of card ranks (0 if pass)
    pub suit: Option<u8>,  // suit of played cards (None if pass)
    pub sequence: u8,      // order within trick (0-3)
}

/// Result of evaluating a trick.
#[derive(Debug, Clone)]
pub struct TrickResult {
    pub winner_seat: Option<u8>,
}

/// Evaluate a completed trick and determine the winner.
///
/// Rules:
/// 1. Collect all non-pass plays
/// 2. If no plays exist, return None (void trick)
/// 3. Trump plays beat non-trump plays regardless of value
/// 4. Among same category: highest value wins
/// 5. Ties: lowest sequence number wins (first player)
pub fn evaluate_trick(
    plays: &[TrickPlayInfo],
    trump_suit: Option<u8>,
    led_suit: Option<u8>,
) -> TrickResult {
    // Filter out passes
    let active_plays: Vec<&TrickPlayInfo> = plays.iter().filter(|p| !p.is_pass).collect();

    if active_plays.is_empty() {
        return TrickResult { winner_seat: None };
    }

    let mut best: Option<&TrickPlayInfo> = None;

    for play in &active_plays {
        let dominated = match best {
            None => true,
            Some(current_best) => play_beats(play, current_best, trump_suit, led_suit),
        };
        if dominated {
            best = Some(play);
        }
    }

    TrickResult {
        winner_seat: best.map(|p| p.seat),
    }
}

/// Returns true if `challenger` beats `current_best`.
fn play_beats(
    challenger: &TrickPlayInfo,
    current_best: &TrickPlayInfo,
    trump_suit: Option<u8>,
    led_suit: Option<u8>,
) -> bool {
    let c_is_trump = is_trump_play(challenger, trump_suit);
    let b_is_trump = is_trump_play(current_best, trump_suit);
    let c_is_led = is_led_suit_play(challenger, led_suit);
    let b_is_led = is_led_suit_play(current_best, led_suit);

    // Trump beats non-trump
    if c_is_trump && !b_is_trump {
        return true;
    }
    if !c_is_trump && b_is_trump {
        return false;
    }

    // Both trump: compare values
    if c_is_trump && b_is_trump {
        return if challenger.play_value > current_best.play_value {
            true
        } else if challenger.play_value == current_best.play_value {
            challenger.sequence < current_best.sequence // earlier wins ties
        } else {
            false
        };
    }

    // Neither is trump
    // Led-suit play beats non-led-suit play
    if c_is_led && !b_is_led {
        return true;
    }
    if !c_is_led && b_is_led {
        return false;
    }

    // Both led suit (or both off-suit — off-suit plays can't win,
    // but we still pick a "best" for consistency)
    if challenger.play_value > current_best.play_value {
        true
    } else if challenger.play_value == current_best.play_value {
        challenger.sequence < current_best.sequence
    } else {
        false
    }
}

fn is_trump_play(play: &TrickPlayInfo, trump_suit: Option<u8>) -> bool {
    match (play.suit, trump_suit) {
        (Some(ps), Some(ts)) => ps == ts,
        _ => false,
    }
}

fn is_led_suit_play(play: &TrickPlayInfo, led_suit: Option<u8>) -> bool {
    match (play.suit, led_suit) {
        (Some(ps), Some(ls)) => ps == ls,
        _ => false,
    }
}

/// Check if a player has any cards of the given suit in their hand.
/// `hand` is a slice of (suit, rank) tuples.
pub fn has_suit(hand: &[(u8, u8)], suit: u8) -> bool {
    hand.iter().any(|(s, _)| *s == suit)
}

/// Determine the next player to act after `current_seat`, skipping
/// players with no cards. Returns None if no player can act (all out).
/// `cards_remaining` is indexed by seat (0-3).
pub fn next_active_seat(current_seat: u8, cards_remaining: &[u8; 4]) -> Option<u8> {
    let mut seat = types::next_seat(current_seat);
    for _ in 0..4 {
        if cards_remaining[seat as usize] > 0 {
            return Some(seat);
        }
        seat = types::next_seat(seat);
    }
    None
}

/// Determine who leads the next trick after a winner.
/// If the winner has no cards, transfer to partner.
/// If partner has no cards, return None (opposing pair sweeps).
pub fn resolve_leader(winner_seat: u8, cards_remaining: &[u8; 4]) -> Option<u8> {
    if cards_remaining[winner_seat as usize] > 0 {
        return Some(winner_seat);
    }
    let partner = types::partner_seat(winner_seat);
    if cards_remaining[partner as usize] > 0 {
        return Some(partner);
    }
    // Neither in the winning pair has cards — opponents auto-win remaining
    None
}

/// Check if the declaring pair has achieved the required spread.
/// Returns Some(true) if contract made, Some(false) if contract failed,
/// None if still in progress.
pub fn check_spread(
    ns_tricks: u8,
    ew_tricks: u8,
    contract_spread: i8,
    declarer_seat: u8,
    total_possible_tricks: u8,
) -> Option<bool> {
    let declarer_is_ns = types::is_ns(declarer_seat);
    let (decl_tricks, def_tricks) = if declarer_is_ns {
        (ns_tricks, ew_tricks)
    } else {
        (ew_tricks, ns_tricks)
    };

    let current_spread = decl_tricks as i8 - def_tricks as i8;
    let tricks_played = ns_tricks + ew_tricks;
    let remaining = total_possible_tricks.saturating_sub(tricks_played);

    // Contract achieved
    if current_spread >= contract_spread {
        return Some(true);
    }
    // Even if declarer wins all remaining, can't make it
    let best_possible = current_spread + remaining as i8;
    if best_possible < contract_spread {
        return Some(false);
    }

    None // Still in play
}

/// Count cards remaining for each seat, given some predicate on card location.
/// This is a helper signature — actual implementation will use the DB.
/// Returns [north_count, east_count, south_count, west_count]
pub fn count_cards_per_seat(cards: &[(u8, u8, u8)]) -> [u8; 4] {
    // cards: (seat, suit, rank) — only cards in hand
    let mut counts = [0u8; 4];
    for (seat, _, _) in cards {
        counts[*seat as usize] += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_play(seat: u8, value: u16, suit: Option<u8>, seq: u8) -> TrickPlayInfo {
        TrickPlayInfo {
            seat,
            is_pass: false,
            play_value: value,
            suit,
            sequence: seq,
        }
    }

    fn make_pass(seat: u8, seq: u8) -> TrickPlayInfo {
        TrickPlayInfo {
            seat,
            is_pass: true,
            play_value: 0,
            suit: None,
            sequence: seq,
        }
    }

    // ---- evaluate_trick tests ----

    #[test]
    fn test_simple_trick_highest_card_wins() {
        let plays = vec![
            make_play(NORTH, 14, Some(SPADES), 0), // Ace of spades
            make_play(EAST, 13, Some(SPADES), 1),   // King of spades
            make_play(SOUTH, 12, Some(SPADES), 2),  // Queen
            make_play(WEST, 11, Some(SPADES), 3),   // Jack
        ];
        let result = evaluate_trick(&plays, None, Some(SPADES));
        assert_eq!(result.winner_seat, Some(NORTH));
    }

    #[test]
    fn test_trump_beats_led_suit() {
        let plays = vec![
            make_play(NORTH, 14, Some(SPADES), 0),  // Ace of spades (led)
            make_play(EAST, 2, Some(HEARTS), 1),     // 2 of hearts (trump)
            make_play(SOUTH, 13, Some(SPADES), 2),   // King of spades
            make_play(WEST, 12, Some(SPADES), 3),    // Queen of spades
        ];
        let result = evaluate_trick(&plays, Some(HEARTS), Some(SPADES));
        assert_eq!(result.winner_seat, Some(EAST));
    }

    #[test]
    fn test_higher_trump_beats_lower_trump() {
        let plays = vec![
            make_play(NORTH, 14, Some(SPADES), 0),   // Ace of spades (led)
            make_play(EAST, 2, Some(HEARTS), 1),      // 2 of hearts (trump)
            make_play(SOUTH, 5, Some(HEARTS), 2),     // 5 of hearts (trump)
            make_play(WEST, 12, Some(SPADES), 3),     // Queen of spades
        ];
        let result = evaluate_trick(&plays, Some(HEARTS), Some(SPADES));
        assert_eq!(result.winner_seat, Some(SOUTH));
    }

    #[test]
    fn test_combination_play_sum_wins() {
        // Player plays 10+5 = 15 of diamonds vs Ace (14) of diamonds
        let plays = vec![
            make_play(NORTH, 14, Some(DIAMONDS), 0),  // Ace of diamonds
            make_pass(EAST, 1),
            make_pass(SOUTH, 2),
            make_play(WEST, 15, Some(DIAMONDS), 3),   // 10d + 5d = 15
        ];
        let result = evaluate_trick(&plays, None, Some(DIAMONDS));
        assert_eq!(result.winner_seat, Some(WEST));
    }

    #[test]
    fn test_all_pass_returns_none() {
        let plays = vec![
            make_pass(NORTH, 0),
            make_pass(EAST, 1),
            make_pass(SOUTH, 2),
            make_pass(WEST, 3),
        ];
        let result = evaluate_trick(&plays, None, None);
        assert_eq!(result.winner_seat, None);
    }

    #[test]
    fn test_tie_first_player_wins() {
        let plays = vec![
            make_play(NORTH, 10, Some(SPADES), 0),
            make_play(EAST, 10, Some(SPADES), 1),   // same value, but later
            make_pass(SOUTH, 2),
            make_pass(WEST, 3),
        ];
        let result = evaluate_trick(&plays, None, Some(SPADES));
        assert_eq!(result.winner_seat, Some(NORTH));
    }

    #[test]
    fn test_off_suit_play_loses_to_led_suit() {
        let plays = vec![
            make_play(NORTH, 5, Some(SPADES), 0),     // 5 of spades (led)
            make_play(EAST, 14, Some(DIAMONDS), 1),    // Ace of diamonds (off-suit, no trump)
            make_pass(SOUTH, 2),
            make_pass(WEST, 3),
        ];
        let result = evaluate_trick(&plays, None, Some(SPADES));
        assert_eq!(result.winner_seat, Some(NORTH));
    }

    #[test]
    fn test_no_trump_game_highest_led_suit_wins() {
        let plays = vec![
            make_play(NORTH, 10, Some(CLUBS), 0),
            make_play(EAST, 14, Some(CLUBS), 1),
            make_play(SOUTH, 8, Some(CLUBS), 2),
            make_play(WEST, 3, Some(CLUBS), 3),
        ];
        // trump_suit = None means No Trump
        let result = evaluate_trick(&plays, None, Some(CLUBS));
        assert_eq!(result.winner_seat, Some(EAST));
    }

    #[test]
    fn test_single_player_wins_others_pass() {
        let plays = vec![
            make_play(NORTH, 2, Some(HEARTS), 0),
            make_pass(EAST, 1),
            make_pass(SOUTH, 2),
            make_pass(WEST, 3),
        ];
        let result = evaluate_trick(&plays, None, Some(HEARTS));
        assert_eq!(result.winner_seat, Some(NORTH));
    }

    #[test]
    fn test_trump_combination_beats_single_trump() {
        // One player plays 2 of trump, another plays 3+4=7 of trump
        let plays = vec![
            make_play(NORTH, 10, Some(SPADES), 0),    // 10 of spades (led)
            make_play(EAST, 2, Some(HEARTS), 1),       // 2 of hearts (trump)
            make_play(SOUTH, 7, Some(HEARTS), 2),      // 3h+4h = 7 (trump combo)
            make_pass(WEST, 3),
        ];
        let result = evaluate_trick(&plays, Some(HEARTS), Some(SPADES));
        assert_eq!(result.winner_seat, Some(SOUTH));
    }

    // ---- next_active_seat tests ----

    #[test]
    fn test_next_active_normal() {
        let cards = [5, 5, 5, 5];
        assert_eq!(next_active_seat(NORTH, &cards), Some(EAST));
    }

    #[test]
    fn test_next_active_skip_empty() {
        let cards = [5, 0, 5, 5]; // East has no cards
        assert_eq!(next_active_seat(NORTH, &cards), Some(SOUTH));
    }

    #[test]
    fn test_next_active_all_empty() {
        let cards = [0, 0, 0, 0];
        assert_eq!(next_active_seat(NORTH, &cards), None);
    }

    #[test]
    fn test_next_active_wraps_around() {
        let cards = [5, 0, 0, 0]; // Only North has cards
        assert_eq!(next_active_seat(WEST, &cards), Some(NORTH));
    }

    // ---- resolve_leader tests ----

    #[test]
    fn test_resolve_leader_winner_has_cards() {
        let cards = [5, 5, 5, 5];
        assert_eq!(resolve_leader(NORTH, &cards), Some(NORTH));
    }

    #[test]
    fn test_resolve_leader_winner_empty_partner_leads() {
        let cards = [0, 5, 5, 5]; // North empty
        assert_eq!(resolve_leader(NORTH, &cards), Some(SOUTH)); // partner
    }

    #[test]
    fn test_resolve_leader_both_empty_opponents_sweep() {
        let cards = [0, 5, 0, 5]; // N and S empty
        assert_eq!(resolve_leader(NORTH, &cards), None);
    }

    // ---- check_spread tests ----

    #[test]
    fn test_spread_achieved() {
        // Declarer is North (NS), bid +3, NS has 5 tricks, EW has 2
        assert_eq!(check_spread(5, 2, 3, NORTH, 13), Some(true));
    }

    #[test]
    fn test_spread_impossible() {
        // Declarer is North, bid +5, NS=1, EW=6, remaining=6
        // Best case: 1+6=7, 7-6=1 < 5 => impossible
        assert_eq!(check_spread(1, 6, 5, NORTH, 13), Some(false));
    }

    #[test]
    fn test_spread_still_possible() {
        // Declarer is North, bid +3, NS=2, EW=1, remaining=10
        assert_eq!(check_spread(2, 1, 3, NORTH, 13), None);
    }

    #[test]
    fn test_spread_ew_declarer() {
        // Declarer is East (EW team), bid +2, NS=1, EW=3
        assert_eq!(check_spread(1, 3, 2, EAST, 13), Some(true));
    }

    // ---- has_suit tests ----

    #[test]
    fn test_has_suit_true() {
        let hand = vec![(SPADES, 14), (HEARTS, 10), (CLUBS, 5)];
        assert!(has_suit(&hand, SPADES));
    }

    #[test]
    fn test_has_suit_false() {
        let hand = vec![(SPADES, 14), (HEARTS, 10)];
        assert!(!has_suit(&hand, DIAMONDS));
    }
}
