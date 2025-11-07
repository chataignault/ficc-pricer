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
- https://wiki.haskell.org/Learn_Haskell_in_10_minutes
- https://www.adit.io/posts/2013-04-17-functors,_applicatives,_and_monads_in_pictures.html
- https://learnyouahaskell.com/a-fistful-of-monads



**FX roadmap :**
- [ ] price futures (amend),
- [ ] price quanto,

**Rates roadmap :**

Linear :
- [x] price Forward Rate Agreement (FRA) from replication portfolio,
- [x] price swap from replication,

Non-linear : (with choice of underlying probabilistic model)
- [ ] price swaption,
- [ ] price cap, floor, caplet, floorlet,

Rate models :
- [ ] LMM,
- [ ] SABR,

Short term rate models for $r_t$ :
- [x] Vasicek,
- [x] Vasicek Hull-White extended,
- [ ] G2 ++,
- [ ] CIR ++,
- [ ] CIR Hull-White extended,

**Keep in mind :**
- the foward and backward Kolmogorov equations,
- ergodicity, ergodic theorem,
- the example of Langevin dynamics,

**FX**
- FOR / DOM change of numeraire,
- currency pair duality - GBM assumption,
- trio of currencies and related measures,

**Rates**
- forward probability change of numeraire 

Which rate :
- Simple rate : $C \mapsto C(1 + (T-t) L (t, T)$, like LIBOR-like values are defined,
- Continuous rate : $C \mapsto Ce^{(T-t)R(t, T)}$
- Compounded rate : $C \mapsto C(1 + Y(t, T))^{(T-t)}$

Which modelisation :
- instantaneous forward rate : $f(t,T) = -\partial_T \log B_t(T)$
- short term rate : $r_t = \partial_t B_t(T)$

***

## FX Pricer

Compositional algebra for pricing FX derivatives in Haskell using algebraic data types.

**Features:**
- **Contracts:** Spot, Forward, European Options, Zero-Coupon Bonds
- **Combinators:** Scale, combine, conditional execution with type safety (GADTs)
- **Pricing:** Black-Scholes implementation with discount factors
- **Design:** Build complex derivatives from basic building blocks with compile-time correctness

See `fx_pricer/` for implementation details and examples.

- [ ] Branch [Quantlib](https://github.com/lballabio/QuantLib/tree/master) and compile for numerical verification, 


## Overview

### Interest rate definitions
- Linear, anually compounded, continuously compounded
- spot or forward

### Short rate models
- Vasicek
- Extended Vasicek Hull-White
- CIR
- General Hull-White
- CIR++
- Black Karasinski

### Forward rate models
- Heath-Jarrow-Morton
- Jamshidian decomposition (swaption pricing)
- LIBOR
- Extended LMM :
  - Levy, 
  - SABR, 
  - shifted,
  - multiple-curve

