module FX.Algebra.Combinators
  ( -- * Smart constructors for common FX products
    fxSpot
  , fxForward
  , fxOption
  , knockInOption
  , knockOutOption
  , digitalOption
  , straddle
  , strangle
    -- * Re-exports
  , module FX.Algebra.Contract
  , module FX.Algebra.Observable
  ) where

import FX.Algebra.Contract
import FX.Algebra.Observable

-- | FX spot exchange
fxSpot :: Currency -> Currency -> Contract
fxSpot = Spot

-- | FX forward constructed via covered interest parity
-- Equivalent to: buy foreign ZCB, sell domestic ZCB, exchange spot
fxForward :: Date -> Currency -> Currency -> Contract
fxForward maturity ccyDom ccyFor =
  Spot ccyDom ccyFor
    <> ZCB ccyFor maturity
    <> Scale (-1) (ZCB ccyDom maturity)

-- | European FX option
fxOption :: OptionType -> Strike -> Date -> Currency -> Currency -> Contract
fxOption = EurOption

-- | Knock-in barrier option
-- Only becomes active if spot rate crosses the barrier
knockInOption
  :: Direction     -- ^ Barrier direction (Up or Down)
  -> Level         -- ^ Barrier level
  -> OptionType    -- ^ Call or Put
  -> Strike        -- ^ Strike price
  -> Date          -- ^ Expiry
  -> Currency      -- ^ Domestic currency
  -> Currency      -- ^ Foreign currency
  -> Contract
knockInOption dir level optType strike expiry ccyDom ccyFor =
  let spotObs = SpotRate (show ccyDom) (show ccyFor)
      barrierHit = Barrier dir level spotObs
  in When barrierHit (EurOption optType strike expiry ccyDom ccyFor)

-- | Knock-out barrier option
-- Becomes worthless if spot rate crosses the barrier
knockOutOption
  :: Direction     -- ^ Barrier direction (Up or Down)
  -> Level         -- ^ Barrier level
  -> OptionType    -- ^ Call or Put
  -> Strike        -- ^ Strike price
  -> Date          -- ^ Expiry
  -> Currency      -- ^ Domestic currency
  -> Currency      -- ^ Foreign currency
  -> Contract
knockOutOption dir level optType strike expiry ccyDom ccyFor =
  let -- Knock-out = vanilla - knock-in
      vanilla = EurOption optType strike expiry ccyDom ccyFor
      knockIn = knockInOption dir level optType strike expiry ccyDom ccyFor
  in Combine vanilla (Scale (-1) knockIn)

-- | Digital (binary) option - pays 1 unit if condition is met
digitalOption
  :: OptionType    -- ^ Call or Put determines the condition
  -> Strike        -- ^ Strike level
  -> Date          -- ^ Expiry
  -> Currency      -- ^ Domestic currency
  -> Currency      -- ^ Foreign currency
  -> Contract
digitalOption optType strike expiry ccyDom ccyFor =
  let spotObs = SpotRate (show ccyDom) (show ccyFor)
      condition = case optType of
        Call -> Barrier Up strike spotObs
        Put  -> Barrier Down strike spotObs
  in When condition (ZCB ccyDom expiry)

-- | Straddle - long both call and put at same strike
straddle :: Strike -> Date -> Currency -> Currency -> Contract
straddle strike expiry ccyDom ccyFor =
  EurOption Call strike expiry ccyDom ccyFor
    <> EurOption Put strike expiry ccyDom ccyFor

-- | Strangle - long call at higher strike, long put at lower strike
strangle
  :: Strike        -- ^ Lower strike (for put)
  -> Strike        -- ^ Higher strike (for call)
  -> Date          -- ^ Expiry
  -> Currency      -- ^ Domestic currency
  -> Currency      -- ^ Foreign currency
  -> Contract
strangle lowerStrike upperStrike expiry ccyDom ccyFor =
  EurOption Put lowerStrike expiry ccyDom ccyFor
    <> EurOption Call upperStrike expiry ccyDom ccyFor