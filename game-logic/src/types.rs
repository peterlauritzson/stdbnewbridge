/// Shared types and enums for the card game.

/// Suit constants: Clubs=0, Diamonds=1, Hearts=2, Spades=3
pub const CLUBS: u8 = 0;
pub const DIAMONDS: u8 = 1;
pub const HEARTS: u8 = 2;
pub const SPADES: u8 = 3;

/// Number of suits
pub const NUM_SUITS: u8 = 4;

/// Seat constants: North=0, East=1, South=2, West=3
pub const NORTH: u8 = 0;
pub const EAST: u8 = 1;
pub const SOUTH: u8 = 2;
pub const WEST: u8 = 3;

/// Number of seats
pub const NUM_SEATS: u8 = 4;

/// Game phase constants (u8 to avoid string mismatch bugs)
pub const PHASE_LOBBY: u8 = 0;
pub const PHASE_AUCTION: u8 = 1;
pub const PHASE_PLAY: u8 = 2;
pub const PHASE_FINISHED: u8 = 3;

/// Lead-after-bid options
pub const LEAD_LEFT_OF_DECLARER: &str = "LeftOfDeclarer";
pub const LEAD_DECLARER: &str = "Declarer";

/// All-pass trick behavior
pub const ALL_PASS_VOID: &str = "VoidTrick";
pub const ALL_PASS_LEADER_WINS: &str = "LeaderWins";

/// Returns the next seat clockwise: N->E->S->W->N
pub fn next_seat(seat: u8) -> u8 {
    (seat + 1) % NUM_SEATS
}

/// Returns the partner's seat (N<->S, E<->W)
pub fn partner_seat(seat: u8) -> u8 {
    (seat + 2) % NUM_SEATS
}

/// Returns true if two seats are on the same team (N-S or E-W)
pub fn same_team(a: u8, b: u8) -> bool {
    (a % 2) == (b % 2)
}

/// Returns true if the seat is on the N-S team
pub fn is_ns(seat: u8) -> bool {
    seat % 2 == 0
}

/// Suit ranking for bidding: Clubs=0 < Diamonds=1 < Hearts=2 < Spades=3 < NT=4
pub fn bid_outranks(new_spread: i8, new_suit: Option<u8>, old_spread: i8, old_suit: Option<u8>) -> bool {
    if new_spread > old_spread {
        return true;
    }
    if new_spread < old_spread {
        return false;
    }
    // Same spread: compare suit rank (None = NT = 4)
    let new_rank = new_suit.map_or(4u8, |s| s);
    let old_rank = old_suit.map_or(4u8, |s| s);
    new_rank > old_rank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_seat() {
        assert_eq!(next_seat(NORTH), EAST);
        assert_eq!(next_seat(EAST), SOUTH);
        assert_eq!(next_seat(SOUTH), WEST);
        assert_eq!(next_seat(WEST), NORTH);
    }

    #[test]
    fn test_partner_seat() {
        assert_eq!(partner_seat(NORTH), SOUTH);
        assert_eq!(partner_seat(SOUTH), NORTH);
        assert_eq!(partner_seat(EAST), WEST);
        assert_eq!(partner_seat(WEST), EAST);
    }

    #[test]
    fn test_same_team() {
        assert!(same_team(NORTH, SOUTH));
        assert!(same_team(EAST, WEST));
        assert!(!same_team(NORTH, EAST));
        assert!(!same_team(SOUTH, WEST));
    }

    #[test]
    fn test_bid_outranks() {
        // Higher spread always wins
        assert!(bid_outranks(2, Some(CLUBS), 1, Some(SPADES)));
        // Same spread, higher suit wins
        assert!(bid_outranks(1, Some(SPADES), 1, Some(HEARTS)));
        // Same spread, NT (None) beats any suit
        assert!(bid_outranks(1, None, 1, Some(SPADES)));
        // Same spread, lower suit loses
        assert!(!bid_outranks(1, Some(CLUBS), 1, Some(DIAMONDS)));
        // Lower spread loses
        assert!(!bid_outranks(1, Some(SPADES), 2, Some(CLUBS)));
    }
}
