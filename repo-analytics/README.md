# repo-analytics

Standalone Rust library for repo market analytics. It sits beside the Haskell
pricer and the Go market gateway and does not depend on the contract algebra.

```bash
cargo test
cargo run --example repo_report

# market data from the Go market-gateway (optional `serde` feature)
cargo test --features serde
cargo run --features serde --example repo_report_from_snapshot           # golden snapshot
(cd ../market-gateway && go run ./cmd/market-gateway repo export --asof 2026-11-09) \
  | cargo run --features serde --example repo_report_from_snapshot -- -
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
| `market`    | *(feature `serde`)* Load and validate market-gateway JSON snapshots into the types above |

## Market snapshots from the gateway

`market-gateway repo export` writes a versioned JSON snapshot
(`schema_version: 1`, schema in `market-gateway/internal/market/repo.go`):

| Section     | Key      | Content |
|-------------|----------|---------|
| `asof`      |          | Business date the curve pillars are counted from |
| `gc_curves` | currency | `day_count`, `pillars[{tenor, days, rate}]`, simple decimal rates |
| `specials`  | ISIN     | `spread` below GC in the bond's currency |
| `fixings`   | index    | `currency`, `day_count`, `fixings[{date, rate}]`, all dated on or before `asof` |
| `bonds`     | ISIN     | `currency`, `coupon_rate`, `frequency`, `maturity`, `face`, `day_count` |
| `prices`    | ISIN     | `clean` price per `face` (the crate computes accrued interest itself) |
| `haircuts`  | ISIN     | `{"kind": "haircut", "value": 0.02}` or `{"kind": "initial_margin", "value": 1.02}` |

`RepoMarket::from_json` / `from_reader` rejects unknown fields and schema
versions, along with inconsistent data. It also checks that each pillar's
`days` equals `Tenor::end_date(asof) - asof`, so Go and Rust cannot drift
apart on tenor arithmetic. Lookups for an unknown ISIN, currency or index
return `MissingMarketData` instead of silently falling back to GC.
`RepoMarket::repo_trade` builds a `RepoTrade` starting at `asof` from the
snapshot price, haircut and GC-minus-special rate.

Both sides test against the same golden files in
`market-gateway/internal/market/testdata/`, using `tests/gateway_snapshot.rs`
here. After a deliberate schema change, regenerate them with
`go test ./internal/market -update` and re-run `cargo test --features serde`.

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
