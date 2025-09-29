module FX.Pricing.BlackScholes
  ( blackScholesPrice
  , normalCDF
  ) where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Time (diffDays)
import FX.Algebra.Contract (Currency, Strike, Date, OptionType(..))
import FX.Pricing.MarketData (MarketState(..), PricingParams(..))

-- | Price a European FX option using Black-Scholes formula
blackScholesPrice
  :: OptionType
  -> Strike
  -> Date
  -> Currency
  -> Currency
  -> MarketState
  -> PricingParams
  -> Double
blackScholesPrice optType strike expiry ccy1 ccy2 market params =
  let valDate = valuationDate params
      tau = yearFrac valDate expiry

      -- Get market data
      spot = getSpotRate market ccy1 ccy2
      vol = getVol market ccy1 ccy2 strike expiry
      dfDom = getDiscountFactor market ccy1 expiry
      dfFor = getDiscountFactor market ccy2 expiry

      -- Forward rate
      forward = spot * (dfFor / dfDom)

      -- Black-Scholes formula
      d1 = (log (forward / strike) + 0.5 * vol * vol * tau) / (vol * sqrt tau)
      d2 = d1 - vol * sqrt tau

      -- Option value in foreign currency
      valueInFor = case optType of
        Call -> dfFor * (forward * normalCDF d1 - strike * normalCDF d2)
        Put  -> dfFor * (strike * normalCDF (-d2) - forward * normalCDF (-d1))

      -- Convert to numeraire currency
      numRate = getSpotRate market ccy2 (numeraire params)

  in if tau <= 0
     then intrinsicValue optType spot strike
     else valueInFor * numRate

-- | Intrinsic value for immediate exercise
intrinsicValue :: OptionType -> Double -> Strike -> Double
intrinsicValue Call spot strike = max 0 (spot - strike)
intrinsicValue Put spot strike = max 0 (strike - spot)

-- | Cumulative distribution function for standard normal
normalCDF :: Double -> Double
normalCDF x = 0.5 * (1.0 + erf (x / sqrt 2.0))

-- | Error function approximation (Abramowitz and Stegun)
erf :: Double -> Double
erf x
  | x < 0     = -erf (-x)
  | otherwise = 1.0 - erfcApprox x
  where
    erfcApprox z =
      let t = 1.0 / (1.0 + 0.5 * z)
          tau = t * exp (- z * z - 1.26551223 +
                  t * (1.00002368 +
                  t * (0.37409196 +
                  t * (0.09678418 +
                  t * (-0.18628806 +
                  t * (0.27886807 +
                  t * (-1.13520398 +
                  t * (1.48851587 +
                  t * (-0.82215223 +
                  t * 0.17087277)))))))))
      in tau

-- Helper functions

getSpotRate :: MarketState -> Currency -> Currency -> Double
getSpotRate market ccy1 ccy2
  | ccy1 == ccy2 = 1.0
  | otherwise = Map.findWithDefault fallbackRate (ccy1, ccy2) (spotRates market)
  where
    fallbackRate = case Map.lookup (ccy2, ccy1) (spotRates market) of
      Just r  -> 1.0 / r
      Nothing -> error $ "Spot rate not found: " ++ show ccy1 ++ "/" ++ show ccy2

getDiscountFactor :: MarketState -> Currency -> Date -> Double
getDiscountFactor market ccy date =
  case Map.lookup ccy (discountCurves market) of
    Just curve -> curve date
    Nothing    -> error $ "Discount curve not found for: " ++ show ccy

getVol :: MarketState -> Currency -> Currency -> Strike -> Date -> Double
getVol market ccy1 ccy2 strike date =
  case Map.lookup (ccy1, ccy2) (volSurfaces market) of
    Just surface -> surface strike date
    Nothing      -> error $ "Vol surface not found: " ++ show ccy1 ++ "/" ++ show ccy2

yearFrac :: Date -> Date -> Double
yearFrac d1 d2 = fromIntegral (diffDays d2 d1) / 365.0