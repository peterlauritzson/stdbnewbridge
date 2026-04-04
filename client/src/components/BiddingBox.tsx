import { suitSymbol, trumpName } from '../lib/cardUtils';
import { CLUBS, DIAMONDS, HEARTS, SPADES } from '../types';
import type { Bid } from '../types';

interface BiddingBoxProps {
  isMyTurn: boolean;
  currentHighBid: Bid | null;
  onBid: (spread: number, suit: number | null) => void;
  onPass: () => void;
}

const SUITS = [CLUBS, DIAMONDS, HEARTS, SPADES, null] as const; // null = NT
const MAX_SPREAD = 26;

function canBid(spread: number, suit: number | null, highBid: Bid | null): boolean {
  if (!highBid || highBid.spread == null) return true;
  if (spread > highBid.spread) return true;
  if (spread === highBid.spread) {
    // suit order: C < D < H < S < NT
    const suitRank = (s: number | null) => s === null ? 4 : s;
    return suitRank(suit) > suitRank(highBid.suit ?? null);
  }
  return false;
}

export function BiddingBox({ isMyTurn, currentHighBid, onBid, onPass }: BiddingBoxProps) {
  // Start showing from current bid spread level (or 1)
  const startSpread = currentHighBid?.spread != null ? currentHighBid.spread : 1;
  const spreads: number[] = [];
  for (let s = startSpread; s <= MAX_SPREAD; s++) spreads.push(s);

  return (
    <div className={`bidding-box ${isMyTurn ? 'bidding-active' : 'bidding-inactive'}`}>
      <h3>Auction</h3>
      <div className="bid-grid-scroll">
        <div className="bid-grid">
          {spreads.map(spread => (
            <div key={spread} className="bid-row">
              <span className="bid-spread">{spread}</span>
              {SUITS.map((suit, i) => {
                const ok = isMyTurn && canBid(spread, suit, currentHighBid);
                return (
                  <button
                    key={i}
                    className={`btn bid-btn ${ok ? '' : 'bid-disabled'}`}
                    disabled={!ok}
                    onClick={() => onBid(spread, suit)}
                    title={`${spread} ${trumpName(suit)}`}
                  >
                    {suit === null ? 'NT' : suitSymbol(suit)}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>
      <button
        className="btn btn-pass"
        disabled={!isMyTurn}
        onClick={onPass}
      >
        Pass
      </button>
    </div>
  );
}

interface BidHistoryProps {
  bids: Bid[];
  seatLabels: Record<number, string>;
}

export function BidHistory({ bids, seatLabels }: BidHistoryProps) {
  if (bids.length === 0) return null;

  return (
    <div className="bid-history">
      <h4>Bid History</h4>
      <table>
        <tbody>
          {bids.map(bid => (
            <tr key={bid.bidId}>
              <td>{seatLabels[bid.seat] ?? `Seat ${bid.seat}`}</td>
              <td>
                {bid.spread == null
                  ? 'Pass'
                  : `${bid.spread} ${trumpName(bid.suit ?? null)}`}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
