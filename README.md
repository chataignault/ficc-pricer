# FX Pricer

Compositional algebra for pricing FX instruments in Haskell.

## Overview

Minimalist pricing library using algebraic data types to construct complex FX derivatives from basic building blocks with type safety.

## Features

- **Algebraic Contracts**: Zero, Spot, Forward, European Options, Zero-Coupon Bonds
- **Observables**: Market rates, barriers, conditional logic
- **Combinators**: Scale, combine, conditional execution
- **Pricing**: Black-Scholes implementation with discount factors
- **Type Safety**: GADTs ensure correctness at compile time

## Build

```bash
cabal build
cabal test
cabal run fx-pricer-example
```

## Structure

- `src/FX/Algebra/` - Contract and observable definitions
- `src/FX/Pricing/` - Pricing engine and market data
- `src/FX/Examples/` - Example products
- `app/` - Executable examples
- `test/` - Test suite

## Example

```haskell
-- EUR/USD forward
forward = Forward settlementDate rate EUR USD

-- European call option
option = EurOption Call 1.10 expiryDate EUR USD

-- Combined portfolio
portfolio = Combine forward option
```

## References
- https://en.wikipedia.org/wiki/Generalized_algebraic_data_type

