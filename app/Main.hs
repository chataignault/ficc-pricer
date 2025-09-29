module Main where

import Data.Map.Strict (Map)
import qualified Data.Map.Strict as Map
import Data.Time
import FX.Algebra.Combinators
import FX.Pricing.Core (price)
import FX.Pricing.MarketData
import FX.Examples.Products

main :: IO ()
main = do
  putStrLn "=== FX Pricing Algebra Demo ==="
  putStrLn ""

  -- Setup market data and pricing parameters
  let market = createMarketData
      params = createPricingParams

  putStrLn "Market Data:"
  putStrLn $ "  EUR/USD Spot: " ++ show (spotRates market Map.! (EUR, USD))
  putStrLn $ "  GBP/USD Spot: " ++ show (spotRates market Map.! (GBP, USD))
  putStrLn $ "  Volatility (EUR/USD): 10%"
  putStrLn ""

  -- Example 1: Simple spot exchange
  putStrLn "=== Example 1: Spot Exchange ==="
  let spotContract = fxSpot EUR USD
  putStrLn $ "Contract: Exchange 1 EUR for USD"
  putStrLn $ "Value: $" ++ show (price spotContract market params)
  putStrLn ""

  -- Example 2: FX Forward
  putStrLn "=== Example 2: FX Forward ==="
  let maturity = fromGregorian 2025 12 31
      fwdContract = Scale 1000000 (fxForward maturity EUR USD)
  putStrLn $ "Contract: 1M EUR/USD forward maturing " ++ show maturity
  putStrLn $ "Value: $" ++ show (price fwdContract market params)
  putStrLn ""

  -- Example 3: European Call Option
  putStrLn "=== Example 3: European Call Option ==="
  let callContract = Scale 500000 (fxOption Call 1.15 maturity EUR USD)
  putStrLn $ "Contract: 500K EUR call USD put, strike 1.15"
  putStrLn $ "Value: $" ++ show (price callContract market params)
  putStrLn ""

  -- Example 4: Call Spread
  putStrLn "=== Example 4: Call Spread ==="
  let spreadContract = Scale 1000000 (callSpread 1.10 1.20 maturity EUR USD)
  putStrLn $ "Contract: 1M EUR call spread (long 1.10, short 1.20)"
  putStrLn $ "Value: $" ++ show (price spreadContract market params)
  putStrLn ""

  -- Example 5: Straddle
  putStrLn "=== Example 5: Straddle ==="
  let straddleContract = Scale 750000 (straddle 1.10 maturity EUR USD)
  putStrLn $ "Contract: 750K straddle at 1.10 strike"
  putStrLn $ "Value: $" ++ show (price straddleContract market params)
  putStrLn ""

  -- Example 6: Complex Portfolio
  putStrLn "=== Example 6: Complex Portfolio ==="
  let portfolio = examplePortfolio maturity
  putStrLn "Portfolio containing:"
  putStrLn "  - Long 1M EUR/USD forward"
  putStrLn "  - Long 500K EUR call @ 1.10"
  putStrLn "  - Short 500K EUR call @ 1.20"
  putStrLn "  - Long 250K knock-in call @ 1.10 (barrier 1.15)"
  putStrLn $ "Total Value: $" ++ show (price portfolio market params)
  putStrLn ""

  -- Example 7: Risk Reversal
  putStrLn "=== Example 7: Risk Reversal ==="
  let rrContract = Scale 1000000 (riskReversal 1.05 1.15 maturity EUR USD)
  putStrLn $ "Contract: 1M risk reversal (long call @ 1.15, short put @ 1.05)"
  putStrLn $ "Value: $" ++ show (price rrContract market params)
  putStrLn ""

  -- Example 8: Demonstrate Algebraic Laws
  putStrLn "=== Example 8: Algebraic Laws ==="
  let c1 = fxOption Call 1.10 maturity EUR USD
      c2 = fxOption Put 1.10 maturity EUR USD
      combined = Combine c1 c2
      p1 = price c1 market params
      p2 = price c2 market params
      pCombined = price combined market params

  putStrLn "Demonstrating: price(c1 <> c2) = price(c1) + price(c2)"
  putStrLn $ "  price(call) = $" ++ show p1
  putStrLn $ "  price(put)  = $" ++ show p2
  putStrLn $ "  price(call <> put) = $" ++ show pCombined
  putStrLn $ "  sum = $" ++ show (p1 + p2)
  putStrLn $ "  difference = $" ++ show (abs (pCombined - (p1 + p2)))
  putStrLn ""

  putStrLn "Demo completed!"

-- | Create sample market data
createMarketData :: MarketState
createMarketData = MarketState
  { spotRates = Map.fromList
      [ ((EUR, USD), 1.10)
      , ((GBP, USD), 1.25)
      , ((USD, JPY), 110.0)
      , ((EUR, GBP), 0.88)
      , ((CHF, USD), 1.05)
      , ((AUD, USD), 0.70)
      , ((CAD, USD), 0.75)
      ]
  , discountCurves = Map.fromList
      [ (USD, constantDF 0.05)
      , (EUR, constantDF 0.03)
      , (GBP, constantDF 0.04)
      , (JPY, constantDF 0.01)
      , (CHF, constantDF 0.02)
      , (AUD, constantDF 0.06)
      , (CAD, constantDF 0.045)
      ]
  , volSurfaces = Map.fromList
      [ ((EUR, USD), constantVol 0.10)
      , ((GBP, USD), constantVol 0.12)
      , ((USD, JPY), constantVol 0.08)
      , ((CHF, USD), constantVol 0.09)
      ]
  , correlations = Map.empty
  }
  where
    -- Constant discount factor curve (simple exponential)
    constantDF :: Double -> Date -> Double
    constantDF r d =
      let t = yearFrac (fromGregorian 2025 1 1) d
      in exp (- r * t)

    -- Constant volatility surface
    constantVol :: Double -> Strike -> Date -> Double
    constantVol v _ _ = v

    yearFrac :: Day -> Day -> Double
    yearFrac d1 d2 = fromIntegral (diffDays d2 d1) / 365.0

-- | Create pricing parameters
createPricingParams :: PricingParams
createPricingParams = PricingParams
  { valuationDate = fromGregorian 2025 1 1
  , numeraire = USD
  , model = BlackScholes
  }