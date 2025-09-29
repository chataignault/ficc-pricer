module FX.Pricing.Core
  ( price
  , evalObservable
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Time (Day, diffDays)
import FX.Algebra.Contract
import FX.Algebra.Observable
import FX.Pricing.MarketData
import FX.Pricing.BlackScholes (blackScholesPrice)

-- | Main pricing function - respects algebraic structure
price :: Contract -> MarketState -> PricingParams -> Double
price contract market params = case contract of
  -- Identity law: Zero has no value
  Zero -> 0.0

  -- Spot FX exchange: convert 1 unit of ccy1 to ccy2
  Spot ccy1 ccy2 ->
    let rate = getSpotRate market ccy1 ccy2
        numRate = getSpotRate market ccy2 (numeraire params)
    in rate * numRate

  -- Forward contract at fixed rate
  Forward maturity fixedRate ccy1 ccy2 ->
    let timeToMaturity = yearFrac (valuationDate params) maturity
        fwdRate = getForwardRate market ccy1 ccy2 maturity
        df = getDiscountFactor market (numeraire params) maturity
        -- Value = (forward rate - fixed rate) * discount factor
    in (fwdRate - fixedRate) * df

  -- European option - delegate to Black-Scholes
  EurOption optType strike expiry ccy1 ccy2 ->
    blackScholesPrice optType strike expiry ccy1 ccy2 market params

  -- Zero coupon bond: discount factor in the given currency
  ZCB ccy maturity ->
    let df = getDiscountFactor market ccy maturity
        fxRate = getSpotRate market ccy (numeraire params)
    in df * fxRate

  -- Scaling law: distribute scalar multiplication
  Scale alpha c ->
    alpha * price c market params

  -- Combination law: addition is pricing under portfolio combination
  Combine c1 c2 ->
    price c1 market params + price c2 market params

  -- Conditional execution
  When obs c ->
    let condition = evalObservable obs market (valuationDate params)
    in if condition
       then price c market params
       else 0.0

-- | Evaluate an observable given market state and evaluation date
evalObservable :: Observable a -> MarketState -> Date -> a
evalObservable obs market date = case obs of
  Const a -> a

  SpotRate ccy1 ccy2 ->
    getSpotRate market (read ccy1) (read ccy2)

  FwdRate ccy1 ccy2 fwdDate ->
    getForwardRate market (read ccy1) (read ccy2) fwdDate

  Barrier dir level obsRate ->
    let rate = evalObservable obsRate market date
    in case dir of
         Up   -> rate >= level
         Down -> rate <= level

  Map f obsA ->
    f (evalObservable obsA market date)

  Apply obsF obsA ->
    let f = evalObservable obsF market date
        a = evalObservable obsA market date
    in f a

-- Helper functions for market data access

getSpotRate :: MarketState -> Currency -> Currency -> Double
getSpotRate market ccy1 ccy2
  | ccy1 == ccy2 = 1.0
  | otherwise = Map.findWithDefault fallbackRate (ccy1, ccy2) (spotRates market)
  where
    -- Try inverse rate if direct rate not available
    fallbackRate = case Map.lookup (ccy2, ccy1) (spotRates market) of
      Just r  -> 1.0 / r
      Nothing -> error $ "Spot rate not found: " ++ show ccy1 ++ "/" ++ show ccy2

getDiscountFactor :: MarketState -> Currency -> Date -> Double
getDiscountFactor market ccy date =
  case Map.lookup ccy (discountCurves market) of
    Just curve -> curve date
    Nothing    -> error $ "Discount curve not found for: " ++ show ccy

getForwardRate :: MarketState -> Currency -> Currency -> Date -> Double
getForwardRate market ccy1 ccy2 date =
  let spot = getSpotRate market ccy1 ccy2
      df1 = getDiscountFactor market ccy1 date
      df2 = getDiscountFactor market ccy2 date
      -- Forward via covered interest rate parity: F = S * (df2/df1)
  in spot * (df2 / df1)

-- | Calculate year fraction between two dates (ACT/365 convention)
yearFrac :: Day -> Day -> Double
yearFrac d1 d2 = fromIntegral (diffDays d2 d1) / 365.0