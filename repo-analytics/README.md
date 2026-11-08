# repo-analytics

Standalone Rust library for repo market analytics. It sits beside the Haskell
pricer and the Go market gateway and does not depend on the contract algebra.

```bash
cargo test
cargo run --example repo_report
```

## Modules

| Module      | Content |
|-------------|---------|
| `daycount`  | ACT/360, ACT/365F, 30/360 US |
| `bond`      | Fixed-coupon collateral: schedule, accrued, price ↔ yield, modified duration, DV01 |
| `trade`     | Classic repo: collateral MV, cash amount, repo interest, repurchase price, open repos |
| `carry`     | Forward price via repo, implied repo rate, carry |
| `risk`      | Mark-to-market, margin calls (threshold / MTA), repo & collateral DV01, specialness |
| `curve`     | GC repo curve from tenors, DFs, forward repo, compounded O/N, per-ISIN specials |

## Formulas and conventions

Prices are per 100 face. Repo rates are simple, annualised, on the trade's day count.

- Cash amount: `MV·(1−h)` with a haircut `h`, or `MV/m` with an initial margin `m`, where MV is the dirty value.
- Repurchase price: `cash·(1 + r·τ)`. Coupons paid during the term are manufactured back to the collateral giver, so they do not change it.
- Forward dirty price: `F = P·(1 + r·τ) − Σ cᵢ·(1 + r·τᵢ)`, where the coupons cᵢ fall in the term and are reinvested at repo to the forward date.
- Implied repo: `r = (F − P + Σcᵢ) / (P·τ − Σ cᵢ·τᵢ)`, the exact inverse of the formula above.
- Carry: `(AI_end − AI_start + Σcᵢ) − P·r·τ`. The forward clean price is the breakeven sale price.
- Margin call: `required − current collateral value`, where `required = (cash + accrued interest)/(1−h)`. Only the part beyond the threshold is called, and only if it is at least the minimum transfer amount.
- Curve: linear interpolation on simple rates with flat extrapolation; `DF = 1/(1 + r·τ)`. Forward = `(DF₁/DF₂ − 1)/τ₁₂`.
- Compounded overnight: `Π(1 + rᵢ·nᵢ/basis) − 1`, annualised. A fixing holds over weekends and holidays.
- Bond accrual: share of the coupon period elapsed (ACT/ACT ICMA when the bond day count is ACT). Yields use street convention, compounded at the coupon frequency.

Not modelled: business-day calendars, settlement lags, tri-party baskets, bond-futures basis.
