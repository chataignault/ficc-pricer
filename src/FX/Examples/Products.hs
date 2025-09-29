module FX.Examples.Products
  ( examplePortfolio
  , riskReversal
  , callSpread
  , putSpread
  , butterfly
  , seagull
  ) where

import FX.Algebra.Combinators

-- | Example portfolio combining multiple FX products
examplePortfolio :: Date -> Contract
examplePortfolio maturity =
  -- Long 1M EUR/USD forward
  Scale 1000000 (fxForward maturity EUR USD)
    -- Long 500K EUR call USD put, strike 1.10
    <> Scale 500000 (fxOption Call 1.10 maturity EUR USD)
    -- Short 500K EUR call USD put, strike 1.20 (covered call)
    <> Scale (-500000) (fxOption Call 1.20 maturity EUR USD)
    -- Long 250K knock-in option (activates if EUR/USD goes above 1.15)
    <> Scale 250000 (knockInOption Up 1.15 Call 1.10 maturity EUR USD)

-- | Risk reversal - long call, short put
-- Used to express directional view with reduced cost
riskReversal :: Strike -> Strike -> Date -> Currency -> Currency -> Contract
riskReversal putStrike callStrike expiry ccyDom ccyFor =
  fxOption Call callStrike expiry ccyDom ccyFor
    <> Scale (-1) (fxOption Put putStrike expiry ccyDom ccyFor)

-- | Call spread - long call at lower strike, short call at higher strike
-- Limited profit, limited risk bullish strategy
callSpread
  :: Strike        -- ^ Lower strike (long)
  -> Strike        -- ^ Higher strike (short)
  -> Date
  -> Currency
  -> Currency
  -> Contract
callSpread lowerStrike upperStrike expiry ccyDom ccyFor =
  fxOption Call lowerStrike expiry ccyDom ccyFor
    <> Scale (-1) (fxOption Call upperStrike expiry ccyDom ccyFor)

-- | Put spread - long put at higher strike, short put at lower strike
-- Limited profit, limited risk bearish strategy
putSpread
  :: Strike        -- ^ Higher strike (long)
  -> Strike        -- ^ Lower strike (short)
  -> Date
  -> Currency
  -> Currency
  -> Contract
putSpread upperStrike lowerStrike expiry ccyDom ccyFor =
  fxOption Put upperStrike expiry ccyDom ccyFor
    <> Scale (-1) (fxOption Put lowerStrike expiry ccyDom ccyFor)

-- | Butterfly spread - profits from low volatility around middle strike
butterfly
  :: Strike        -- ^ Lower strike
  -> Strike        -- ^ Middle strike
  -> Strike        -- ^ Upper strike
  -> Date
  -> Currency
  -> Currency
  -> Contract
butterfly k1 k2 k3 expiry ccyDom ccyFor =
  fxOption Call k1 expiry ccyDom ccyFor
    <> Scale (-2) (fxOption Call k2 expiry ccyDom ccyFor)
    <> fxOption Call k3 expiry ccyDom ccyFor

-- | Seagull spread - combines risk reversal with knock-out
-- Long call, short put, knock-out at upper barrier
seagull
  :: Strike        -- ^ Put strike (short)
  -> Strike        -- ^ Call strike (long)
  -> Level         -- ^ Knock-out barrier
  -> Date
  -> Currency
  -> Currency
  -> Contract
seagull putStrike callStrike koLevel expiry ccyDom ccyFor =
  knockOutOption Up koLevel Call callStrike expiry ccyDom ccyFor
    <> Scale (-1) (fxOption Put putStrike expiry ccyDom ccyFor)