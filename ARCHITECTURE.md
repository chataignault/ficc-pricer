# FX Pricer Service Architecture

## Overview

Distributed pricing service where:
- **Go application**: Orchestrates pricing requests and manages market data updates
- **Haskell service**: Performs FX derivative pricing computations
- **Communication**: gRPC with Protocol Buffers
- **Latency target**: <50ms end-to-end

```
┌─────────────┐                           ┌──────────────────┐
│             │   gRPC (Protobuf)         │                  │
│ Go Client   │──────────────────────────>│ Haskell Service  │
│             │   Price Requests          │  (FX Pricer)     │
│ - Market    │<──────────────────────────│                  │
│   data mgmt │   Price Responses         │ - Contract       │
│ - Request   │                           │   algebra        │
│   orchestr. │   Market Updates          │ - Black-Scholes  │
│             │──────────────────────────>│   pricing        │
└─────────────┘                           └──────────────────┘
```

---

## Protocol Selection: gRPC + Protobuf

### Rationale

| Criteria | JSON/HTTP | gRPC + Protobuf | Kafka + Avro |
|----------|-----------|-----------------|--------------|
| **Latency** | ~100ms | ~10-20ms ⭐ | ~200ms+ |
| **Type Safety** | Runtime validation | Schema-enforced ⭐ | Schema-enforced |
| **Operational Complexity** | Low ⭐ | Low ⭐ | High (cluster mgmt) |
| **Go Support** | Excellent | Excellent ⭐ | Good |
| **Haskell Support** | Excellent | Good ⭐ | Limited |
| **Streaming** | No | Bidirectional ⭐ | Yes |
| **Infrastructure** | None ⭐ | None ⭐ | Kafka + ZooKeeper |

**Decision**: gRPC + Protobuf offers the best balance for point-to-point, low-latency communication without infrastructure overhead.

---

## Service Interface Definition

### RPC Methods

```protobuf
service FXPricer {
  // Synchronous pricing request
  rpc Price(PriceRequest) returns (PriceResponse);

  // Update market data (spots, curves, vols)
  rpc UpdateMarket(MarketUpdate) returns (Ack);

  // Optional: Health check
  rpc Health(Empty) returns (HealthStatus);
}
```

---

## Message Structure: Inputs

### 1. Price Request

**Purpose**: Request price for a single contract or portfolio

**Structure**:
```protobuf
message PriceRequest {
  Contract contract = 1;           // Contract to price
  MarketSnapshot market = 2;       // Market data snapshot
  PricingParams params = 3;        // Valuation parameters
}
```

**Fields**:

#### `Contract` (Recursive GADT encoding)

Maps to Haskell's `Contract` GADT from `src/FX/Algebra/Contract.hs:29`:

```protobuf
message Contract {
  oneof contract_type {
    Zero zero = 1;
    Spot spot = 2;
    Forward forward = 3;
    EurOption eur_option = 4;
    ZCB zcb = 5;
    Scale scale = 6;
    Combine combine = 7;
    When when = 8;
  }
}

// Individual contract types
message Zero {}

message Spot {
  Currency domestic = 1;
  Currency foreign = 2;
}

message Forward {
  string maturity = 1;      // ISO 8601 date (e.g., "2025-12-31")
  double fixed_rate = 2;
  Currency domestic = 3;
  Currency foreign = 4;
}

message EurOption {
  OptionType option_type = 1;
  double strike = 2;
  string maturity = 3;      // ISO 8601 date
  Currency domestic = 4;
  Currency foreign = 5;
}

message ZCB {
  Currency currency = 1;
  string maturity = 2;      // ISO 8601 date
}

message Scale {
  double notional = 1;
  Contract contract = 2;    // Recursive reference
}

message Combine {
  Contract left = 1;        // Recursive reference
  Contract right = 2;       // Recursive reference
}

message When {
  Observable condition = 1;
  Contract contract = 2;    // Recursive reference
}

// Enumerations
enum Currency {
  USD = 0;
  EUR = 1;
  GBP = 2;
  JPY = 3;
  CHF = 4;
  AUD = 5;
  CAD = 6;
}

enum OptionType {
  CALL = 0;
  PUT = 1;
}
```

**Example encoding** (EUR/USD call option):
```json
// Conceptual JSON representation
{
  "contract_type": {
    "scale": {
      "notional": 1000000,
      "contract": {
        "eur_option": {
          "option_type": "CALL",
          "strike": 1.15,
          "maturity": "2025-12-31",
          "domestic": "USD",
          "foreign": "EUR"
        }
      }
    }
  }
}
```

---

#### `MarketSnapshot` (Market data at a point in time)

Maps to Haskell's `MarketState` from `src/FX/Pricing/MarketData.hs:14`:

```protobuf
message MarketSnapshot {
  map<string, double> spot_rates = 1;           // Key: "EUR/USD", Value: 1.10
  map<string, DiscountCurve> discount_curves = 2; // Key: "USD"
  map<string, VolSurface> vol_surfaces = 3;      // Key: "EUR/USD"
  map<string, double> correlations = 4;          // Key: "EUR/USD/GBP/USD"
}
```

**Challenge**: Haskell uses `Date -> Double` functions for curves. We need serializable representation.

**Solution**: Encode curves as either flat rates or pillar points with interpolation method:

```protobuf
message DiscountCurve {
  oneof curve_type {
    FlatRate flat_rate = 1;
    PillarCurve pillar_curve = 2;
  }
}

message FlatRate {
  double rate = 1;              // Constant rate (e.g., 0.05 for 5%)
  string compounding = 2;       // "continuous", "annual", "semiannual"
}

message PillarCurve {
  repeated DateValue points = 1;  // Curve pillars
  string interpolation = 2;        // "linear", "cubic", "flat_forward"
}

message DateValue {
  string date = 1;              // ISO 8601 date
  double value = 2;             // Discount factor or rate
}
```

**Volatility surface encoding**:

```protobuf
message VolSurface {
  oneof surface_type {
    FlatVol flat_vol = 1;
    VolGrid vol_grid = 2;
  }
}

message FlatVol {
  double volatility = 1;        // Constant vol (e.g., 0.12 for 12%)
}

message VolGrid {
  repeated VolPoint points = 1;
  string interpolation = 2;      // "linear", "cubic", "SABR"
}

message VolPoint {
  double strike = 1;
  string maturity = 2;          // ISO 8601 date
  double volatility = 3;
}
```

**Haskell reconstruction**:

```haskell
-- Haskell side converts serialized curves back to functions
buildDiscountCurve :: DiscountCurve -> (Date -> Double)
buildDiscountCurve (FlatRate r comp) =
  \d -> exp (-r * dayCountFraction d comp)
buildDiscountCurve (PillarCurve points interp) =
  interpolate interp points
```

---

#### `PricingParams` (Valuation configuration)

Maps to Haskell's `PricingParams` from `src/FX/Pricing/MarketData.hs:26`:

```protobuf
message PricingParams {
  string valuation_date = 1;    // ISO 8601 date (e.g., "2025-01-01")
  Currency numeraire = 2;       // Currency for all output prices
  PricingModel model = 3;       // Pricing model to use
}

enum PricingModel {
  BLACK_SCHOLES = 0;
  LOCAL_VOL = 1;
  HESTON = 2;
}
```

---

### 2. Market Update

**Purpose**: Push market data changes from Go to Haskell (e.g., live spot updates)

**Structure**:
```protobuf
message MarketUpdate {
  oneof update_type {
    SpotUpdate spot_update = 1;
    CurveUpdate curve_update = 2;
    VolUpdate vol_update = 3;
  }
  int64 timestamp_ms = 4;       // Unix timestamp in milliseconds
}

message SpotUpdate {
  string pair = 1;              // "EUR/USD"
  double rate = 2;
}

message CurveUpdate {
  string currency = 1;          // "USD"
  DiscountCurve curve = 2;
}

message VolUpdate {
  string pair = 1;              // "EUR/USD"
  VolSurface surface = 2;
}
```

---

## Message Structure: Outputs

### 1. Price Response

**Purpose**: Return computed price to Go client

**Structure**:
```protobuf
message PriceResponse {
  double price = 1;                 // Price in numeraire currency
  string numeraire = 2;             // Currency of price (redundant but explicit)
  PriceBreakdown breakdown = 3;     // Optional: component breakdown
  double computation_time_ms = 4;   // Performance monitoring
  string error = 5;                 // Empty if success, error message otherwise
}

message PriceBreakdown {
  repeated ComponentPrice components = 1;
}

message ComponentPrice {
  string description = 1;       // e.g., "EUR/USD Call Strike 1.15"
  double price = 2;
}
```

**Example response**:
```json
{
  "price": 42567.89,
  "numeraire": "USD",
  "computation_time_ms": 3.2,
  "error": ""
}
```

---

### 2. Acknowledgment

**Purpose**: Confirm market data update receipt

**Structure**:
```protobuf
message Ack {
  bool success = 1;
  string message = 2;           // Error message or confirmation
  int64 timestamp_ms = 3;       // Server timestamp
}
```

---

### 3. Health Status

**Purpose**: Service health monitoring

**Structure**:
```protobuf
message HealthStatus {
  bool healthy = 1;
  string version = 2;
  int64 uptime_seconds = 3;
  int32 requests_processed = 4;
}

message Empty {}
```

---

## Data Flow

### Scenario 1: Synchronous Pricing

```
Go Client                                   Haskell Service
    |                                             |
    | 1. Construct PriceRequest                   |
    |    - Contract: EUR/USD call                 |
    |    - Market: spots, curves, vols            |
    |    - Params: valuation date, numeraire      |
    |                                             |
    | 2. gRPC call: Price(request)                |
    |-------------------------------------------->|
    |                                             |
    |                                             | 3. Deserialize protobuf
    |                                             | 4. Convert to Haskell types:
    |                                             |    - Contract GADT
    |                                             |    - MarketState
    |                                             |    - PricingParams
    |                                             |
    |                                             | 5. Call pricing engine:
    |                                             |    price :: Contract -> MarketState -> PricingParams -> Double
    |                                             |
    |                                             | 6. Serialize response
    |                                             |
    | 7. Receive PriceResponse                    |
    |<--------------------------------------------|
    |    - price: 42567.89                        |
    |    - computation_time_ms: 3.2               |
    |                                             |
    | 8. Use price in Go application              |
    |                                             |
```

**Latency breakdown**:
- Serialization (Go): ~1ms
- Network: <1ms (localhost) or 5-10ms (LAN)
- Deserialization (Haskell): ~1ms
- Pricing computation: 5-30ms (depends on contract complexity)
- Serialization (Haskell): ~0.5ms
- Network: <1ms or 5-10ms
- Deserialization (Go): ~0.5ms
- **Total**: ~10-50ms

---

### Scenario 2: Market Data Update

```
Go Client                                   Haskell Service
    |                                             |
    | 1. Receive spot update (e.g., from market   |
    |    data provider: EUR/USD = 1.1050)         |
    |                                             |
    | 2. Construct MarketUpdate                   |
    |    - spot_update: "EUR/USD" -> 1.1050       |
    |    - timestamp_ms: 1704099600000            |
    |                                             |
    | 3. gRPC call: UpdateMarket(update)          |
    |-------------------------------------------->|
    |                                             |
    |                                             | 4. Update internal MarketState
    |                                             |    spotRates Map
    |                                             |
    | 5. Receive Ack                              |
    |<--------------------------------------------|
    |    - success: true                          |
    |                                             |
```

**Concurrency consideration**: Haskell service may need to handle:
- Concurrent price requests (read-only on MarketState)
- Concurrent market updates (writes to MarketState)
- Solution: Use `TVar` or `MVar` for thread-safe market state management

---

## Observable Handling

**Challenge**: Your `When` contract uses `Observable Bool` from `src/FX/Algebra/Observable.hs`.

**Protobuf encoding**:

```protobuf
message Observable {
  oneof observable_type {
    ConstBool const_bool = 1;
    SpotRate spot_rate = 2;
    FwdRate fwd_rate = 3;
    Barrier barrier = 4;
    ObservableMap map = 5;
    ObservableApply apply = 6;
  }
}

message ConstBool {
  bool value = 1;
}

message SpotRate {
  Currency domestic = 1;
  Currency foreign = 2;
}

message Barrier {
  Direction direction = 1;
  double level = 2;
  Observable underlying = 3;  // Recursive
}

enum Direction {
  UP = 0;
  DOWN = 1;
}

// Note: Map and Apply are complex to serialize
// May need simplified representation or evaluation on Go side
```

---

## Implementation Phases

### Phase 1: Minimal Viable Service (Week 1)

**Scope**:
- Support basic contract types: `Spot`, `Forward`, `EurOption`, `Scale`, `Combine`
- Flat rate discount curves only
- Flat volatility surfaces only
- No `Observable` support (defer `When` contracts)

**Deliverables**:
1. `pricer.proto` with core messages
2. Go client stub with example price request
3. Haskell server skeleton with `warp` or `grpc-haskell`
4. End-to-end test: price a EUR/USD call option

---

### Phase 2: Full Market Data Support (Week 2)

**Scope**:
- Pillar-based discount curves with interpolation
- Volatility grids with strike/maturity dimensions
- Market update RPC implementation
- Correlation matrix support

**Deliverables**:
1. Extended market data messages
2. Curve/surface interpolation in Haskell
3. Market state management (thread-safe updates)
4. Integration test suite

---

### Phase 3: Advanced Features (Week 3+)

**Scope**:
- `Observable` support for barrier options
- Portfolio pricing (multiple contracts in one request)
- Streaming market updates (gRPC server streaming)
- Performance optimization (lazy evaluation, caching)

---

## Library Dependencies

### Go

```go
// go.mod
module fx-pricer-client

go 1.21

require (
    google.golang.org/grpc v1.60.0
    google.golang.org/protobuf v1.32.0
)
```

**Codegen**: `protoc --go_out=. --go-grpc_out=. pricer.proto`

---

### Haskell

```cabal
-- ficc-pricer.cabal additions
build-depends:
    -- Existing dependencies
    base, containers, time, QuickCheck, ...

    -- New: gRPC and Protobuf
  , proto-lens >= 0.7
  , proto-lens-runtime >= 0.7
  , grpc-haskell >= 0.2        -- gRPC server
  , grpc-haskell-core >= 0.2

    -- Alternative: If grpc-haskell is problematic
  , warp >= 3.3                -- HTTP/2 server
  , http2-grpc-proto-lens      -- gRPC over warp
```

**Codegen**: Use `proto-lens-protoc` plugin

```bash
protoc --plugin=protoc-gen-haskell=`which proto-lens-protoc` \
       --haskell_out=src/ \
       pricer.proto
```

---

## Open Questions / Design Decisions

### 1. Market State Management Strategy

**Options**:
- **Stateless**: Client sends full market snapshot with every request
  - Pros: Simple, no state synchronization
  - Cons: Large messages, redundant data transfer

- **Stateful**: Server maintains market state, client sends updates
  - Pros: Smaller messages, more efficient
  - Cons: State synchronization, crash recovery

**Recommendation**: Start stateless (Phase 1), migrate to stateful (Phase 2) with explicit state versioning.

---

### 2. Interpolation Method

**Question**: Who decides interpolation method for curves?

**Options**:
- Go specifies in message (more flexible)
- Haskell hardcodes (simpler, less configuration)

**Recommendation**: Go specifies, Haskell validates (reject unsupported methods).

---

### 3. Error Handling

**Question**: How to handle pricing errors (e.g., missing market data, invalid contract)?

**Options**:
- gRPC status codes (NOT_FOUND, INVALID_ARGUMENT)
- Error field in response message
- Both

**Recommendation**: Both - gRPC codes for transport errors, error field for pricing logic errors.

---

### 4. Concurrency Model

**Question**: How many concurrent price requests?

**Options**:
- Single-threaded Haskell service (simple, sequential)
- Thread pool (N workers handle requests concurrently)
- Full async with `async` library

**Recommendation**: Thread pool with size = number of CPU cores. Use `TVar` for market state.

---

### 5. Observable Serialization Complexity

**Question**: Full `Observable` support or simplified approach?

**Options**:
- Full GADT encoding (complex, feature-complete)
- Pre-evaluate observables on Go side, send boolean (simple, lossy)
- Defer to Phase 3

**Recommendation**: Defer to Phase 3. Focus on core contracts first.

---

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Latency (p50)** | <50ms | End-to-end Go → Haskell → Go |
| **Latency (p99)** | <100ms | Include GC pauses |
| **Throughput** | >1000 req/s | Single Haskell instance |
| **Message size** | <10KB | Typical contract + market data |
| **Serialization** | <2ms | Protobuf encode/decode |

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| `grpc-haskell` build issues | High | Fallback: HTTP/2 + protobuf without gRPC framework |
| Protobuf schema evolution | Medium | Use optional fields, maintain backward compatibility |
| Curve interpolation mismatch | Medium | Comprehensive test suite with reference values |
| Market state race conditions | High | Use STM (`TVar`) for atomic updates |
| Large contract serialization | Low | Implement size limits, reject oversized contracts |

---

## Next Steps

1. **Review this document** - Validate inputs/outputs match requirements
2. **Create `pricer.proto`** - Define full protobuf schema
3. **Implement Phase 1** - Basic gRPC service with flat market data
4. **Integration test** - Go client → Haskell service roundtrip
5. **Benchmark** - Validate <50ms latency target

---

## Appendix: Example Request/Response

### Example 1: Price a EUR/USD Call Option

**Request (conceptual JSON)**:
```json
{
  "contract": {
    "scale": {
      "notional": 1000000,
      "contract": {
        "eur_option": {
          "option_type": "CALL",
          "strike": 1.15,
          "maturity": "2025-12-31",
          "domestic": "USD",
          "foreign": "EUR"
        }
      }
    }
  },
  "market": {
    "spot_rates": {"EUR/USD": 1.10},
    "discount_curves": {
      "USD": {"flat_rate": {"rate": 0.05, "compounding": "continuous"}},
      "EUR": {"flat_rate": {"rate": 0.03, "compounding": "continuous"}}
    },
    "vol_surfaces": {
      "EUR/USD": {"flat_vol": {"volatility": 0.12}}
    }
  },
  "params": {
    "valuation_date": "2025-01-01",
    "numeraire": "USD",
    "model": "BLACK_SCHOLES"
  }
}
```

**Response**:
```json
{
  "price": 42567.89,
  "numeraire": "USD",
  "computation_time_ms": 3.2,
  "error": ""
}
```

---

### Example 2: Market Update (Spot Rate)

**Request**:
```json
{
  "spot_update": {
    "pair": "EUR/USD",
    "rate": 1.1050
  },
  "timestamp_ms": 1704099600000
}
```

**Response**:
```json
{
  "success": true,
  "message": "Spot rate EUR/USD updated to 1.1050",
  "timestamp_ms": 1704099600123
}
```
