# PRD: Minimalist FX Pricing Algebra in Haskell

## Objective

Build a minimal, compositional algebra for pricing FX instruments in Haskell that enables construction of complex derivatives from basic building blocks while maintaining mathematical rigor and type safety.

## Core Requirements

### 1. Algebraic Data Types

#### Contract Type
```haskell
data Contract where
  Zero      :: Contract                           -- Identity contract
  Spot      :: Currency -> Currency -> Contract   -- Immediate FX exchange
  Forward   :: Date -> Rate -> Currency -> Currency -> Contract
  EurOption :: OptionType -> Strike -> Date -> Currency -> Currency -> Contract
  ZCB       :: Currency -> Date -> Contract       -- Zero coupon bond
  Scale     :: Double -> Contract -> Contract     -- Notional scaling
  Combine   :: Contract -> Contract -> Contract   -- Portfolio combination
  When      :: Observable Bool -> Contract -> Contract  -- Conditional execution
```

#### Observable Type
```haskell
data Observable a where
  Const     :: a -> Observable a
  SpotRate  :: Currency -> Currency -> Observable Double
  FwdRate   :: Currency -> Currency -> Date -> Observable Double
  Barrier   :: Direction -> Level -> Observable Double -> Observable Bool
```

#### Support Types
```haskell
data Currency = USD | EUR | GBP | JPY | ...
data OptionType = Call | Put
data Direction = Up | Down
type Date = Day  -- From Data.Time
type Rate = Double
type Strike = Double
type Level = Double
```

### 2. Mathematical Laws (Type Classes)

#### Monoid Instance for Portfolio Combination
```haskell
instance Monoid Contract where
  mempty = Zero
  mappend = Combine
```

**Laws to satisfy:**
- Associativity: `(a <> b) <> c = a <> (b <> c)`
- Identity: `Zero <> a = a` and `a <> Zero = a`
- Commutativity: `a <> b = b <> a`

#### Functor/Applicative for Observable
```haskell
instance Functor Observable
instance Applicative Observable
```

**Purpose:** Enable composition of market observables with standard operators.

#### Scaling Distributivity
Manual verification required:
- `Scale α (Combine a b) = Combine (Scale α a) (Scale α b)`

### 3. Core Primitives

#### Minimum Required Contracts
1. **Zero**: `Zero :: Contract`
2. **Spot**: `spot :: Currency -> Currency -> Contract`
3. **Forward**: `forward :: Date -> Rate -> Currency -> Currency -> Contract`
4. **European Option**: `eurOption :: OptionType -> Strike -> Date -> Currency -> Currency -> Contract`
5. **Zero Coupon Bond**: `zcb :: Currency -> Date -> Contract`

#### Minimum Required Combinators
1. **Scale**: `scale :: Double -> Contract -> Contract`
2. **Combine**: `(<>) :: Contract -> Contract -> Contract` (via Monoid)
3. **When**: `when :: Observable Bool -> Contract -> Contract`

### 4. Pricing Function

#### Type Signature
```haskell
data MarketState = MarketState
  { spotRates       :: Map (Currency, Currency) Double
  , discountCurves  :: Map Currency (Date -> Double)
  , volSurfaces     :: Map (Currency, Currency) (Strike -> Date -> Double)
  , correlations    :: Map (Currency, Currency, Currency, Currency) Double
  }

data PricingParams = PricingParams
  { valuationDate   :: Date
  , numeraire       :: Currency
  , model           :: PricingModel
  }

data PricingModel = BlackScholes | LocalVol | Heston

price :: Contract -> MarketState -> PricingParams -> Double
```

#### Compositionality Requirement
Pricing must respect algebraic structure:
- `price Zero _ _ = 0`
- `price (Combine a b) m p = price a m p + price b m p`
- `price (Scale α c) m p = α * price c m p`

### 5. Observable Evaluation

```haskell
evalObservable :: Observable a -> MarketState -> Date -> a
```

Observables are pure functions of market state and evaluation time.

### 6. Implementation Constraints

#### Minimalism Requirements
- **No unnecessary abstractions**: Only introduce type classes that encode genuine algebraic laws
- **No over-engineering**: Avoid complex monad stacks or advanced type-level features unless mathematically justified
- **Focus on composition**: Core value is building complex instruments from simple ones

#### Code Organization
```
src/
├── FX/
│   ├── Algebra/
│   │   ├── Contract.hs      -- Core contract ADT
│   │   ├── Observable.hs    -- Observable ADT and instances
│   │   └── Combinators.hs   -- Smart constructors and helpers
│   ├── Pricing/
│   │   ├── Core.hs          -- Pricing function and semantics
│   │   ├── BlackScholes.hs  -- BS model implementation
│   │   └── MarketData.hs    -- Market state types
│   └── Examples/
│       └── Products.hs      -- Example structured products
```

### 7. Deliverables

1. **Core algebra module** with Contract and Observable types
2. **Pricing engine** implementing `price` function for all primitives
3. **Smart constructors** for common FX products:
   - `fxForward :: Date -> Currency -> Currency -> Contract`
   - `fxOption :: OptionType -> Strike -> Date -> Currency -> Currency -> Contract`
   - `knockInOption :: Direction -> Level -> OptionType -> Strike -> Date -> Currency -> Currency -> Contract`
4. **Example portfolio** demonstrating composition
5. **Property tests** verifying algebraic laws (QuickCheck)

### 8. Example Usage

```haskell
-- FX Forward via covered interest parity
fxForward :: Date -> Currency -> Currency -> Contract
fxForward maturity ccy1 ccy2 =
  spot ccy1 ccy2 <> zcb ccy2 maturity <> scale (-1) (zcb ccy1 maturity)

-- Barrier option
knockInOption :: Direction -> Level -> OptionType -> Strike -> Date -> Currency -> Currency -> Contract
knockInOption dir level optType strike expiry ccy1 ccy2 =
  when (barrier dir level (spotRate ccy1 ccy2)) (eurOption optType strike expiry ccy1 ccy2)

-- Portfolio
portfolio :: Contract
portfolio =
  scale 1000000 (fxForward dec2025 USD EUR) <>
  scale 500000 (knockInOption Up 1.15 Call 1.10 dec2025 EUR USD)
```

### 9. Non-Requirements (Out of Scope)

- American option pricing (requires path-dependent evaluation)
- Credit valuation adjustments (CVA/DVA)
- Collateral modeling (CSA agreements)
- Multi-curve discounting (OIS vs Libor)
- Numerical methods implementation details (agent may choose appropriate methods)
- Real-time market data connectivity
- Trade lifecycle management

### 10. Success Criteria

- [ ] All primitives (Zero, Spot, Forward, EurOption, ZCB) implemented
- [ ] Combinators (Scale, Combine, When) working correctly
- [ ] Monoid laws verified via property tests
- [ ] Pricing function handles all contract types
- [ ] At least 3 example structured products built via composition
- [ ] Black-Scholes pricing for European options
- [ ] Code is < 500 lines (excluding tests and examples)

## Technical Notes

- Use `Data.Time` for date handling
- Use `containers` Map for market data lookups
- Leverage Haskell's laziness for deferred evaluation optimization
- Keep dependencies minimal: base, containers, time, QuickCheck
