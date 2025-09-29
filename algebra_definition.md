# Algebra Definition for FX Pricing

## Introduction

In quantitative finance, an **algebra** is a design pattern that provides a compositional framework for building complex financial instruments from simpler building blocks. This algebraic approach enables efficient pricing, risk management, and portfolio construction for FX structured products through the systematic combination of basic contracts, market observables, and combinators.

The algebra paradigm offers several key advantages:
- **Modularity**: Complex instruments are decomposed into reusable components
- **Compositionality**: New instruments can be built by combining existing ones
- **Maintainability**: Changes to basic contracts propagate through the system
- **Performance**: Lazy evaluation and shared computations optimize pricing

## Core Components

### 1. Basic Contracts

Basic contracts represent the fundamental atomic instruments in FX markets:

- **Spot Contract**: Immediate exchange of currencies at current market rate
- **Forward Contract**: Agreement to exchange currencies at a future date and predetermined rate
- **European Option**: Right (but not obligation) to exchange currencies at expiry
- **American Option**: Right to exchange currencies at any time before expiry
- **Zero Coupon Bond**: Discounting instrument for a specific currency and maturity

### 2. Observables

Observables are market data points and derived quantities that drive contract valuations:

- **Spot Rates**: Current exchange rates between currency pairs
- **Forward Rates**: Market-implied future exchange rates
- **Interest Rate Curves**: Yield curves for each currency
- **Volatility Surfaces**: Implied volatilities across strikes and maturities
- **Correlation Matrices**: Cross-currency correlations for multi-asset products
- **Credit Spreads**: Counterparty risk adjustments

### 3. Combinators

Combinators are operations that compose complex instruments from simpler ones:

- **Sequential Composition**: Chaining contracts in time (e.g., swap legs)
- **Parallel Composition**: Combining simultaneous positions
- **Conditional Composition**: State-dependent instrument behavior
- **Scaling**: Adjusting notional amounts and directions
- **Currency Conversion**: Expressing payoffs in different currencies

## Mathematical Foundation

### Algebraic Laws

The pricing algebra satisfies fundamental mathematical properties:

1. **Associativity**: `(A ⊕ B) ⊕ C = A ⊕ (B ⊕ C)`
2. **Commutativity**: `A ⊕ B = B ⊕ A` (for portfolio combination)
3. **Identity**: `A ⊕ 0 = A` (zero contract is identity)
4. **Distributivity**: `α × (A ⊕ B) = (α × A) ⊕ (α × B)` (scaling distributes)

### Composition Rules

- **Temporal Composition**: Sequencing contracts across time periods
- **Spatial Composition**: Combining contracts across different underlyings
- **Conditional Composition**: Path-dependent and barrier-triggered structures

### Pricing Semantics

Each contract in the algebra maps to a pricing function:
```
Price: Contract × Market_State × Pricing_Parameters → Valuation
```

## FX-Specific Applications

### Currency Pair Modeling

The algebra naturally handles the asymmetric nature of FX:
- Base currency and quote currency roles
- Reciprocal relationships between currency pairs
- Cross-rate consistency through triangular arbitrage

### Multi-Currency Instruments

Complex structures involving multiple currencies:
- **Cross-Currency Swaps**: Exchange of cash flows in different currencies
- **Quanto Options**: Options with payoffs in a third currency
- **Rainbow Options**: Multi-asset options on currency baskets

### Structured Products

Building sophisticated FX products:
- **Barrier Options**: Knock-in/knock-out features using conditional combinators
- **Digital Options**: Binary payoffs through threshold observables
- **Exotic Swaps**: Non-standard payment schedules and reset mechanisms

## Implementation Benefits

### Modularity and Reusability

- Components can be developed, tested, and validated independently
- Common patterns emerge across different product types
- Risk factors are naturally isolated and can be hedged systematically

### Risk Management Integration

- Greeks (sensitivities) compose naturally through the algebra
- Scenario analysis can be applied uniformly across all instruments
- Portfolio-level risk metrics aggregate from component risks

### Performance Optimization

- **Lazy Evaluation**: Computations deferred until needed
- **Memoization**: Repeated calculations cached across instruments
- **Parallel Processing**: Independent components evaluated concurrently
- **Incremental Updates**: Market moves trigger minimal recalculations

## Examples

### FX Forward from Spot and Bonds

```
FX_Forward(T, K, CCY1, CCY2) =
    Spot(CCY1, CCY2) × ZCB(CCY2, T) / ZCB(CCY1, T) - K × ZCB(CCY2, T)
```

### Cross-Currency Swap

```
XCCY_Swap(Notional, Floating_Leg, Fixed_Leg) =
    Scale(Notional, Floating_Leg) ⊕ Scale(-Notional, Fixed_Leg)
```

### European Barrier Option

```
Barrier_Option(Spot_Option, Barrier_Level, Direction) =
    Conditional(
        Barrier_Observable(Barrier_Level, Direction),
        Spot_Option,
        Zero_Contract
    )
```

### Portfolio Construction

```
FX_Portfolio =
    Scale(w1, Forward1) ⊕
    Scale(w2, Option2) ⊕
    Scale(w3, Swap3)
```

## Risk Factor Decomposition

The algebraic structure naturally decomposes portfolio risk into:

- **Delta Risk**: First-order sensitivity to spot rate moves
- **Gamma Risk**: Convexity from option positions
- **Vega Risk**: Volatility sensitivity
- **Theta Risk**: Time decay effects
- **Rho Risk**: Interest rate sensitivity for each currency

## Conclusion

The algebraic approach to FX pricing provides a powerful framework for building, pricing, and managing complex currency derivatives. By decomposing instruments into basic contracts, observables, and combinators, the algebra enables systematic construction of sophisticated products while maintaining mathematical rigor and computational efficiency.

This compositional design facilitates:
- Rapid product development and customization
- Consistent risk measurement across instrument types
- Efficient computation through structural optimization
- Clear separation of market data, model parameters, and instrument logic

The algebra serves as both a theoretical foundation and practical implementation guide for modern FX trading systems.